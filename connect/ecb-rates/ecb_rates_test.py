#!/usr/bin/env python3
from __future__ import annotations

import csv
import importlib.util
import io
import json
import os
import pathlib
import sys
import unittest
from datetime import date


APP, MANIFEST, BOOK, SCOPES = map(pathlib.Path, sys.argv[1:5])
sys.path.insert(0, str(APP.parent.parent))
spec = importlib.util.spec_from_file_location("ecb_rates", APP)
assert spec and spec.loader
ecb = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = ecb
spec.loader.exec_module(ecb)


def provider(code: str, value: str, day: str = "2026-09-09") -> str:
    return (
        "TIME_PERIOD,CURRENCY,CURRENCY_DENOM,OBS_VALUE\n"
        f"{day},{code},EUR,{value}\n"
    )


class EcbRatesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.old = dict(os.environ)
        os.environ.update(
            {
                "RATIO_CONNECT_API_URL": "https://connect.example.test",
                "RATIO_API_ORIGIN": "https://demo.example.test",
            }
        )

    def tearDown(self) -> None:
        os.environ.clear()
        os.environ.update(self.old)

    def test_cross_rate_is_decimal_quantized_once_and_retains_raw_observations(self) -> None:
        day = date(2026, 9, 9)
        obs = {
            "EUR": ecb.Observation("EUR", day, ecb.Decimal("1"), "1"),
            "USD": ecb.Observation("USD", day, ecb.Decimal("1.1693"), "1.1693"),
            "GBP": ecb.Observation("GBP", day, ecb.Decimal("0.86545"), "0.86545"),
        }
        text = ecb.normalized_delivery(
            base="USD",
            currencies=["USD", "EUR", "GBP"],
            on_day=day,
            observations=obs,
            today=day,
        )
        rows = list(csv.DictReader(io.StringIO(text)))
        self.assertEqual([r["Currency"] for r in rows], ["EUR", "GBP"])
        self.assertEqual(rows[0]["Rate"], "1.17")
        self.assertEqual(rows[1]["Rate"], "1.35")
        self.assertEqual(rows[1]["SourceRate"], "0.86545")
        self.assertEqual(rows[1]["SourceBase"], "1.1693")

    def test_provider_row_must_match_currency_base_and_requested_day(self) -> None:
        day = date(2026, 9, 9)
        with self.assertRaisesRegex(ecb.Refuse, "exactly one"):
            ecb.parse_observation(provider("USD", "1.2", "2026-09-08"), "USD", day)
        with self.assertRaisesRegex(ecb.Refuse, "exactly one"):
            ecb.parse_observation(provider("GBP", "0.8"), "USD", day)
        with self.assertRaisesRegex(ecb.Refuse, "nonpositive"):
            ecb.parse_observation(provider("USD", "0"), "USD", day)

    def test_future_missing_and_mislabeled_inputs_refuse(self) -> None:
        day = date(2026, 9, 10)
        usd = ecb.Observation("USD", day, ecb.Decimal("1.1"), "1.1")
        with self.assertRaisesRegex(ecb.Refuse, "future"):
            ecb.normalized_delivery(
                base="USD", currencies=["USD", "EUR"], on_day=day,
                observations={"USD": usd}, today=date(2026, 9, 9)
            )
        with self.assertRaisesRegex(ecb.Refuse, "not a declared"):
            ecb.normalized_delivery(
                base="GBP", currencies=["USD", "EUR"], on_day=day,
                observations={"USD": usd}, today=day
            )
        with self.assertRaisesRegex(ecb.Refuse, "missing ECB"):
            ecb.normalized_delivery(
                base="USD", currencies=["USD", "JPY"], on_day=day,
                observations={"USD": usd}, today=day
            )

    def test_sync_reads_the_book_then_ingests_and_admits_without_journal_access(self) -> None:
        calls = []

        def ratio_transport(method, url, headers, body):
            calls.append((method, url, json.loads(body) if body else None))
            if method == "GET":
                return 200, json.dumps(
                    {"kind": "PERSONAL", "currencyCode": "USD", "currencies": ["USD", "EUR"]}
                )
            if url.endswith(":ingest"):
                return 200, '{"newFactCount":"1"}'
            if url.endswith(":admit"):
                return 200, '{"recordedCount":"1"}'
            return 500, ""

        def ecb_transport(url):
            self.assertIn("D.USD.EUR.SP00.A", url)
            return 200, provider("USD", "1.1693")

        ingest, admit = ecb.sync(
            "household", date(2026, 9, 9), token="verified",
            provider_transport=ecb_transport, ratio_transport=ratio_transport,
            today=date(2026, 9, 9),
        )
        self.assertEqual(ingest["newFactCount"], "1")
        self.assertEqual(admit["recordedCount"], "1")
        self.assertEqual(
            [c[1].removeprefix("https://connect.example.test/v1/") for c in calls],
            ["books/household", "funds/household:ingest", "funds/household:admit"],
        )
        self.assertEqual(calls[1][2]["templateId"], "ecb-reference-rates")
        self.assertNotIn(":applyEvent", " ".join(c[1] for c in calls))

    def test_wrong_kind_refuses_before_provider_or_write(self) -> None:
        def ratio_transport(method, url, headers, body):
            return 200, '{"kind":"INVESTMENT","currencyCode":"USD","currencies":["USD","EUR"]}'

        with self.assertRaisesRegex(ecb.Refuse, "Personal book"):
            ecb.sync(
                "fund", date(2026, 9, 9), token="verified",
                provider_transport=lambda _: self.fail("provider called"),
                ratio_transport=ratio_transport, today=date(2026, 9, 9),
            )

    def test_manifest_and_repository_contracts_match(self) -> None:
        app = json.loads(MANIFEST.read_text())
        self.assertEqual(set(app["workos_connect"]["scopes"]), ecb.SCOPES)
        self.assertEqual(app["journal_access"], "none")
        self.assertEqual(app["ingest_template"], ecb.TEMPLATE_ID)
        book = BOOK.read_text()
        self.assertIn('"ecb-reference-rates"', book)
        self.assertIn('id = "ecb-reference-rates"', book)
        scopes = SCOPES.read_text()
        for scope in ecb.SCOPES:
            self.assertIn(f"`{scope}`", scopes)


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]])
