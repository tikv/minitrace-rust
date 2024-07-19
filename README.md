> [!WARNING]
>
> `minitrace` maintainers have forked this project to [`fastrace`](https://github.com/fastracelabs/fastrace). Please follow the migration guide to update your code. **This repository will no longer be maintained unless anyone else takes responsibility**.

Edit your `Cargo.toml` and find and replace `minitrace` with `fastrace` in the source code:

```diff
# Cargo.toml

[dependencies]
- minitrace = "0.6"
+ fastrace = "0.6"
```
