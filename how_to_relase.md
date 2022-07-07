# Release Guideline

1. Bump the version in `Cargo.toml` and `README.md`
2. Run [`cargo semver`](https://github.com/rust-lang/rust-semverver)
3. Make a commit
4. Tag the commit `git tag vX.Y.Z`
5. [Release library to crates.io](https://doc.rust-lang.org/cargo/reference/publishing.html)
