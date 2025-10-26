# libisyntax-rs

Somewhat rusty wrapper around [libisyntax](https://github.com/amspath/libisyntax).

## Example

```rust
use std::env;
use libisyntax_rs::*;

fn main() -> Result<()> {
    let path = env::args().nth(1).expect("Missing path");
    let isyntax = ISyntax::open(path)?;

    let level = isyntax.level(0)?;

    for j in 0..level.height_in_tiles() {
        for i in 0..level.width_in_tiles() {
            let img = level.read_tile(i as i64, j as i64)?;
            img.save(format!("output_tiles/{i}_{j}.png")).unwrap();
        }
    }

    Ok(())
}
```