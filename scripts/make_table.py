#!/usr/bin/env python3
"""Merge raw-<variant>.csv + rss.csv into results/raw.csv and print a markdown table."""
import csv, pathlib
R = str(pathlib.Path(__file__).resolve().parent.parent / "results")
rss = {}
for row in csv.reader(open(f"{R}/rss.csv")):
    v, s, m, rc, kb = row
    rss[(v, s, m)] = (rc, kb)
rows = []
for v in ("patched", "unpatched"):
    try:
        for r in csv.DictReader(open(f"{R}/raw-{v}.csv")):
            r["variant"] = v
            rc, kb = rss.get((v, r["scheme"], r["m"]), ("", ""))
            r["peak_rss_mb"] = f"{int(kb)/1024:.0f}" if kb else ""
            rows.append(r)
    except FileNotFoundError:
        pass
cols = ["variant","scheme","m","call","prover","po2","total_cycles","user_cycles","paging_cycles",
        "segments","prove_s","verify_s","receipt_bytes","peak_rss_mb","verified"]
with open(f"{R}/raw.csv", "w", newline="") as f:
    w = csv.DictWriter(f, fieldnames=cols, extrasaction="ignore"); w.writeheader(); w.writerows(rows)
base = {r["variant"]: int(r["user_cycles"]) for r in rows if r["scheme"] == "noop" and r["m"] == "1"}
print("| ed25519 crate | scheme | M | total cycles | user cycles | user − noop | per-sig (user − noop)/M | segments | prove time | receipt (B) | peak RSS (MB) |")
print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
for r in rows:
    u = int(r["user_cycles"]); m = int(r["m"]); b = base.get(r["variant"], 0)
    d = u - b
    per = "" if r["scheme"] == "noop" else f"{d//m:,}"
    t = float(r["prove_s"])
    var = "risc0 fork (accelerated)" if r["variant"] == "patched" else "crates.io 2.2.0 (as LEZ v0.3.1)"
    print(f"| {var} | `{r['scheme']}` | {m} | {int(r['total_cycles']):,} | {u:,} | {d:,} | {per} | {r['segments']} | {t:.1f} s ({int(t//60)}:{int(t%60):02d}) | {int(r['receipt_bytes']):,} | {r['peak_rss_mb']} |")
