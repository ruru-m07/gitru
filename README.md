<div align="center">
  <img width="128" height="128" alt="Gitru" src="https://github.com/user-attachments/assets/aaa17f7b-f1ff-41c1-8601-a125155bb7d7" />
</div>

## Development

After installing the workspace dependencies, run the same verification suite used by CI from the repository root:

```sh
make verify
```

This runs the desktop frontend tests, checks frontend linting and types, builds the desktop frontend, checks Rust formatting and Clippy warnings, and runs the Rust workspace tests.
