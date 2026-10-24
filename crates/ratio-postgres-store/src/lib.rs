//! Native Postgres immutable objects. See tla/PostgresJournal.tla first.
//! No ledger SQL: the unchanged kernel folds the stored journal bytes.
use anyhow::{bail, ensure, Context, Result};
use fallible_iterator::FallibleIterator;
use postgres::{Client, IsolationLevel, NoTls};
use ratio_store::ObjectStore;
use std::sync::{mpsc::sync_channel, Mutex};

pub struct PgStore {
    client: Mutex<Option<Client>>,
    table: String,
    namespace: String,
}
impl PgStore {
    /// Loopback-only proof/development connection. Hosted callers supply a
    /// TLS-connected native Client through from_client; never downgrade TLS.
    pub fn connect_local(url: &str, schema: &str, namespace: &str) -> Result<Self> {
        let config: postgres::Config = url
            .parse()
            .context("invalid Postgres connection configuration")?;
        ensure!(
            !config.get_hosts().is_empty(),
            "an explicit loopback host is required"
        );
        ensure!(
            config
                .get_hostaddrs()
                .iter()
                .all(std::net::IpAddr::is_loopback),
            "connect_local requires a loopback host address"
        );
        for host in config.get_hosts() {
            match host {
                postgres::config::Host::Tcp(host) if matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") => (),
                _ => bail!("connect_local requires loopback; hosted storage requires a TLS-connected native client"),
            }
        }
        let client = std::thread::spawn(move || config.connect(NoTls))
            .join()
            .map_err(|_| anyhow::anyhow!("Postgres connection worker panicked"))??;
        Self::from_client(client, schema, namespace)
    }
    /// Bind an already authenticated native client to one immutable namespace.
    /// Schema creation is deliberate here; neither a URL nor server is default.
    pub fn from_client(client: Client, schema: &str, namespace: &str) -> Result<Self> {
        let this = Self {
            client: Mutex::new(Some(client)),
            table: format!("{schema}.immutable_objects"),
            namespace: namespace.into(),
        };
        ensure!(
            !schema.is_empty()
                && schema.len() <= 63
                && schema
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "schema must be 1..63 bytes of [a-z0-9_]"
        );
        ensure!(
            !namespace.is_empty()
                && namespace.len() <= 256
                && !namespace.chars().any(char::is_control),
            "invalid object namespace"
        );
        this.call(|client| {
            ensure!(client.query_one("SHOW fsync", &[])?.get::<_, String>(0) == "on",
                "Postgres journal requires fsync=on");
            client.batch_execute("SET synchronous_commit=on")?;
            client.batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}; CREATE TABLE IF NOT EXISTS {} (namespace TEXT NOT NULL, object_key TEXT COLLATE \"C\" NOT NULL, body BYTEA NOT NULL, PRIMARY KEY(namespace, object_key));", this.table))?;
            Ok(())
        })?;
        Ok(this)
    }
    // The synchronous client owns a Tokio runtime. Run its work off any caller
    // runtime, including a current-thread HTTP handler, rather than nesting it.
    fn call<T: Send>(&self, f: impl FnOnce(&mut Client) -> Result<T> + Send) -> Result<T> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let mut client = self
                        .client
                        .lock()
                        .map_err(|_| anyhow::anyhow!("Postgres client lock poisoned"))?;
                    f(client.as_mut().context("Postgres client closed")?)
                })
                .join()
                .map_err(|_| anyhow::anyhow!("Postgres object worker panicked"))?
        })
    }
    fn height_sql(&self) -> String {
        format!("SELECT object_key FROM {} WHERE namespace=$1 AND object_key >= $2 AND starts_with(object_key,$2) AND substring(object_key FROM char_length($2)+1) ~ '^[0-9]{{20}}$' AND object_key <= $3 ORDER BY object_key DESC LIMIT 1", self.table)
    }
    fn sequence(prefix: &str, key: &str) -> Result<u64> {
        key.strip_prefix(prefix)
            .context("sequence escaped its prefix")?
            .parse()
            .context("invalid sequence key")
    }
}
impl ObjectStore for PgStore {
    fn put_if_absent(&self, key: &str, body: &[u8]) -> Result<bool> {
        self.call(|client| {
            // INSERT is one autocommit transaction. A row count is acknowledged
            // only after the driver has received the committed command result.
            Ok(client.execute(&format!("INSERT INTO {} (namespace,object_key,body) VALUES ($1,$2,$3) ON CONFLICT(namespace,object_key) DO NOTHING",self.table), &[&self.namespace,&key,&body])? == 1)
        })
    }
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.call(|client| {
            Ok(client
                .query_opt(
                    &format!(
                        "SELECT body FROM {} WHERE namespace=$1 AND object_key=$2",
                        self.table
                    ),
                    &[&self.namespace, &key],
                )?
                .map(|row| row.get(0)))
        })
    }
    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        self.call(|client| Ok(client.query(&format!("SELECT object_key FROM {} WHERE namespace=$1 AND starts_with(object_key,$2) ORDER BY object_key",self.table), &[&self.namespace,&prefix])?.into_iter().map(|row|row.get(0)).collect()))
    }
    fn max_sequence(&self, prefix: &str) -> Result<u64> {
        self.call(|client| {
            let upper = format!("{prefix}{:020}", u64::MAX);
            let row = client.query_opt(&self.height_sql(), &[&self.namespace, &prefix, &upper])?;
            row.map(|row| Self::sequence(prefix, row.get::<_, &str>(0)))
                .transpose()
                .map(|seq| seq.unwrap_or(0))
        })
    }
    fn scan_sequence(
        &self,
        prefix: &str,
        after: u64,
        f: &mut dyn FnMut(u64, &[u8]) -> Result<()>,
    ) -> Result<u64> {
        // A bounded handoff lets the caller run a non-Send kernel callback
        // while the native client streams on its runtime's dedicated thread.
        let (send, receive) = sync_channel::<(u64, Vec<u8>)>(8);
        std::thread::scope(|scope| {
            let worker=scope.spawn(move || -> Result<u64> {
                let mut client=self.client.lock().map_err(|_|anyhow::anyhow!("Postgres client lock poisoned"))?;
                let mut transaction=client.as_mut().context("Postgres client closed")?.build_transaction().isolation_level(IsolationLevel::RepeatableRead).read_only(true).start()?;
                let upper=format!("{prefix}{:020}",u64::MAX);
                let height=transaction.query_opt(&self.height_sql(), &[&self.namespace,&prefix,&upper])?.map(|row| Self::sequence(prefix,row.get::<_,&str>(0))).transpose()?.unwrap_or(0);
                let lower=format!("{prefix}{after:020}");
                let sql=format!("SELECT object_key,body FROM {} WHERE namespace=$1 AND starts_with(object_key,$2) AND object_key > $3 AND object_key <= $4 AND substring(object_key FROM char_length($2)+1) ~ '^[0-9]{{20}}$' ORDER BY object_key",self.table);
                let params: [&(dyn postgres::types::ToSql+Sync);4]=[&self.namespace,&prefix,&lower,&upper];
                let mut rows=transaction.query_raw(&sql,params)?;
                while let Some(row)=rows.next()? {
                    let seq=Self::sequence(prefix,row.get::<_,&str>(0))?;
                    if send.send((seq,row.get(1))).is_err() { bail!("journal reader refused the stream"); }
                }
                drop(rows);
                transaction.commit()?;
                Ok(height)
            });
            // The worker must own the only sender or iteration could wait for
            // a sender retained by this caller after the database is finished.
            let mut refused = None;
            for (seq, body) in receive.iter() {
                if let Err(error) = f(seq, &body) {
                    refused = Some(error);
                    break;
                }
            }
            drop(receive);
            let result = worker
                .join()
                .map_err(|_| anyhow::anyhow!("Postgres scan worker panicked"))?;
            if let Some(error) = refused {
                return Err(error);
            }
            result
        })
    }
}

impl Drop for PgStore {
    fn drop(&mut self) {
        let client = self
            .client
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        // Dropping the synchronous driver's runtime inside Tokio can panic too.
        let _ = std::thread::spawn(move || drop(client)).join();
    }
}
