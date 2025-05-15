# Security Policy

## Dependency Audit Advisory: `sqlx-mysql` / `rsa` Marvin Attack

**Context:**  

This project uses [`sqlx`](https://github.com/launchbadge/sqlx) for PostgreSQL database access only. MySQL and other database engines are not enabled, as seen in our [`Cargo.toml`](./Cargo.toml):


```
sqlx = { version = "0.8.1", default-features = false, features = ["postgres", "runtime-tokio-native-tls", "macros", "chrono", "migrate"] }
```


Despite this minimal configuration, cargo-audit may detect sqlx-mysql (and its transitive dependency, the vulnerable rsa crate) in our Cargo.lock file, referencing the RUSTSEC-2023-0071 advisory.

**Why This Is Not a Vulnerability For Us**

sqlx-mysql exists only as a potential optional backend inside the SQLx source tree and is included in Cargo.lock for completeness.
We do not enable the default or MySQL features in sqlx, so neither sqlx-mysql nor rsa are ever built, shipped, or made available at runtime.
The Marvin Attack vulnerability in rsa is, therefore, not exploitable or present in any running binaries or distributed code from this project.

**References**

- [RUSTSEC-2023-0071, Marvin Attack on rsa crate](https://rustsec.org/advisories/RUSTSEC-2023-0071)
- [SQLx issue #2353: Cargo audit cannot ignore sqlx-mysql advisories](https://github.com/launchbadge/sqlx/issues/2353)
- [Cargo tracking: unused crates in Cargo.lock](https://github.com/rust-lang/cargo/issues/8832)

**Our Mitigation**

We explicitly ignore this advisory in our audit process with a `.cargo/audit.toml` file:

```
[advisories]
ignore = ["RUSTSEC-2023-0071"]
This configuration is reviewed regularly and will be updated if a future version of SQLx or Cargo changes optional dependency handling.
```

**Contact**

For any security concerns or if you need further explanation, please open an issue or contact the project maintainer.