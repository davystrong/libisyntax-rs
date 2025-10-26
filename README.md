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

## Notes

* `ISyntax` isn't `Send` and making it `Send` caused issues. More work is needed on this.
* The reader cache size (as defined in the original library) is hardcoded to 2000.
* The original library has an ISyntax file, image and level. I've simplified this to just have the file and the level.