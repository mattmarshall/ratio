//! ratio-api — gRPC service surface over the kernel.
//!
//! Phase-0 stub. Phase 2 defines the services as AIP-linted protos
//! (`//proto/ratio/v1`) + a tonic server over `ratio-kernel`.

/// Placeholder marker for the Phase-0 build graph.
pub const CRATE: &str = "ratio-api";

#[cfg(test)]
mod tests {
    #[test]
    fn links_kernel() {
        assert!(ratio_kernel::money_is_zero(ratio_kernel::usd_zero()));
    }
}
