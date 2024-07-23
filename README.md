![](./etc/img/head-img-640.svg)

# minitrace has become [fastrace](https://github.com/fastracelabs/fastrace)!

We decide to continue the development of minitrace under a new organization structure for better community governance.

[fastrace](https://github.com/fastracelabs/fastrace) is maintained by the same maintainers of minitrace, so that we encourage all users to just migrate.

Meanwhile, minitrace will not be maintained any more. See https://github.com/tikv/minitrace-rust/issues/229 for details.

## Migrate to fastrace

Simply substitute the occurance of `minitrace` with `fastrace` in your source code, like:

```diff
# Cargo.toml

[dependencies]
- minitrace = "0.6"
+ fastrace = "0.6"
```
