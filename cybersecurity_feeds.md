# TigerBlue OSINT Dashboard – **Licence-Compliant Feed List**

This documents the _public-demo_ feed set for TigerBlue.app after applying **Option B: “open-licence-only”**.  Every feed below is either U.S.-government public-domain, released under the UK Open Government Licence (OGL v3.0) or published under a permissive Creative Commons licence.  That means you can legally display _headline + short AI summary + link-back_ in a public dashboard with only minimal attribution.

> **Important:** If you enable any additional feeds you _must_ verify the licence terms yourself before deploying them to the public instance.

---

## 📜 Core public feeds

| # | Feed Name | RSS/Atom Link | Focus Area(s) | Licence | Short Coverage Summary |
|---|-----------|---------------|---------------|---------|------------------------|
| 1 | **CISA Cybersecurity Alerts** | <https://us-cert.cisa.gov/ncas/alerts.xml> | Threat alerts | Public Domain (US-Gov) | High-priority alerts on active threats and exploited vulnerabilities from the US Cybersecurity & Infrastructure Security Agency. |
| 2 | **CISA Vulnerability Advisories** | <https://www.cisa.gov/cybersecurity-advisories/all.xml> | Vulnerability disclosures, patches | Public Domain (US-Gov) | Authoritative advisories covering newly disclosed CVEs—including ICS/OT notices—with mitigation guidance and severity ratings. |
| 3 | **UK NCSC Updates** | <https://www.ncsc.gov.uk/api/1/services/v1/all-rss-feed.xml> | National cyber news & guidance | **OGL v3.0** | Advisories, guidance notes and incident reports from the UK’s National Cyber Security Centre. |
| 4 | **SANS Internet Storm Center Diaries** | <https://isc.sans.edu/rssfeed_full.xml> | Daily threat analysis | **CC BY-NC-SA 3.0 US** | Community-run diary posts analysing emerging attack trends, malware and exploits. Requires non-commercial use and attribution. |
| 5 | **CERT-EU Security Advisories** | <https://cert.europa.eu/publications/threat-intelligence-rss> | EU-focused advisories & vulnerabilities | **CC BY 4.0** | Bulletins for EU institutions and agencies on newly disclosed vulnerabilities and threats. |

All five feeds are enabled in `Cargo.toml` and ingested hourly by default (or at the feed’s own `<ttl>` cadence if provided).

---

## 🚧 Feeds removed from the public demo

| Reason | Examples |
|--------|----------|
| Proprietary **“all rights reserved”** content | Krebs on Security, Dark Reading, BleepingComputer, The Hacker News, Microsoft MSRC, Cisco Talos, etc. |
| Licenced only for **personal use** via RSS | Exploit-DB, Threatpost, Malwarebytes Labs |
| Unclear or mixed user-generated copyright | Reddit /r/netsec |

These feeds remain in the codebase (parsers & unit tests) but are commented out in `Cargo.toml`.  Organisations may enable them privately at their own risk.

---

## ➕ Optional open feeds (commented-out examples)

If you need more coverage without licence headaches, uncomment and test any of the following:

| Feed | Link | Licence | What you gain |
|------|------|---------|---------------|
| **CISA Known Exploited Vulnerabilities (KEV)** | <https://www.cisa.gov/sites/default/files/feeds/known_exploited_vulnerabilities.xml> | Public Domain | Live list of CVEs confirmed to be exploited in the wild. |
| **Australian ACSC Alerts** | <https://www.cyber.gov.au/alerts/rss.xml> | CC BY 4.0 AU | Southern-hemisphere advisories and incident notes. |
| **CERT-NZ Advisories** | <https://www.cert.govt.nz/it-specialists/rss-advisories/> | CC BY 4.0 NZ | Additional vulnerability and incident context. |
| **NVD CVE JSON 2.0** | <https://services.nvd.nist.gov/rest/json/cves/2.0?recent=true> | Public Domain | Machine-readable CVE data for enrichment (bulk API). |
| **MITRE ATT&CK Changelog** | <https://github.com/mitre/cti/releases.atom> | Royalty-free | Detects new/renamed techniques for mapping enrichment. |

---

## 👣 Attribution requirements

* **CISA & other US-Gov feeds:** no legal attribution required, but a courtesy line such as `© CISA (US Government, public domain)` is good practice.
* **UK NCSC (OGL v3.0):** must include `© Crown copyright 20XX, NCSC. Contains public-sector information licensed under the Open Government Licence v3.0.`
* **SANS ISC (CC BY-NC-SA):** non-commercial use only; include `© SANS Internet Storm Center – CC BY-NC-SA 3.0 US` with a link back.
* **CERT-EU (CC BY 4.0):** include `© CERT-EU – CC BY 4.0` + link.

The dashboard displays this attribution in the article card footer and again in the `/sources` page.

---

## 🛠 Adding a new feed

1. **Check the licence** – look for CC, OGL, US-Gov PD, or equivalent.  When in doubt, e-mail the publisher.
2. **Update `Cargo.toml`** – add `licence = "…"` and appropriate tags.
3. **Limit excerpt** – the public cache stores **≤ 100 characters** of AI-generated summary and the original headline only.
4. **Run `cargo test`** – ensure the parser and licence checker pass.
5. **Deploy** – watch logs for `LicenceWarning` on first ingest.

---

## Citations & Licence sources

1. **CISA Terms of Use (public domain)** – <https://www.cisa.gov/about/tou>  
2. **UK Open Government Licence v3.0** – <https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/>  
3. **SANS ISC Licence Notice** – Diary page footer, e.g. <https://isc.sans.edu/>  
4. **CERT-EU RSS page** – <https://cert.europa.eu/about>  

_All links accessed 29 May 2025._

---

Happy hacking!  If you spot any licence drift, open an issue or e-mail **legal@tigerblue.app**.
