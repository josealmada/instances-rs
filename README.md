# Instances-rs

[![CI](https://github.com/josealmada/instances-rs/actions/workflows/general.yml/badge.svg?branch=main)](https://github.com/josealmada/instances-rs/actions/workflows/general.yml)

## Disclaimer

This crate is being made for learning purposes. Any feedback is welcome.

## Overview

`instances-rs` is a crate for making the application aware of how
many of its instances its online.

## Usage

To use `instances-rs`, add the following to your Cargo.toml: **[NOT PUBLISHED YET]**
```toml
[dependencies]
instances-rs = "0.1"
```

Then build the `Instances` object that will start updating the current instance info
and fetching the other ones data. (See [usability_test.rs](/tests/usability_test.rs)
for example)

```rust
use instances_rs::config::Builder;
use std::time::Duration;

fn main() {
    let instances_rs = Builder::default()
        .with_update_interval(Duration::from_secs(10))
        .with_backend(...) // One backend of your choose
        .with_info_extractor(...) // Some function that will extract the current instance data that you want to publish
        .build();

    // Optionally you can wait for
    instances_rs
        .wait_for_first_update(Duration::from_secs(10))
        .unwrap();

    // To get the info about the current instance
    instances_rs.get_instance_info();

    // To get the instance count
    instances_rs.instances_count();

    // List all active instances
    instances_rs.list_active_instances();
}
```

### Data extractor

You can choose wherever data you like to publish with your instance data. The only
restriction is that it must be `Serializable` and `Deserializable`.

Example:

````rust
    .with_info_extractor(|| env::consts::OS.to_string())
````

### Backends

You can choose one of the available backends to store the instances' data or implement
your own.

**Right now there is no backend implemented, but I have plans to implement the following
alternatives: MySQL, DynamoDB and Redis.**

### Leader strategy

You can classify your instances choosing one `LeaderStrategy`. By default
`LeaderStrategy::None` is used.

### Error strategy

You can choose one `CommunicationErrorStrategy` to handle error on updates.

* `CommunicationErrorStrategy::Error` will make the update return an `Err`to
the daemon and all the instances' info will be unavailable.
* `CommunicationErrorStrategy::UseLastInfo` will emit a warning during the update
and the outdated data will still be available.


## License

This project is licensed under the [MIT license](LICENSE).