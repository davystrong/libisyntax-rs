use std::{env, sync::mpsc::sync_channel, thread};

use image::RgbaImage;
use libisyntax_sys::*;

fn main() -> Result<()> {
    let path = env::args().nth(1).expect("Missing path");
    let isyntax = ISyntax::open(path)?;

    // let img = isyntax.read_macro_image()?;
    // let img = isyntax.label_image()?;

    let level = isyntax.level(0)?;

    let (tx, rx) = sync_channel::<(i32, i32, RgbaImage)>(16);

    let handle = thread::spawn(|| {
        for (i, j, img) in rx {
            img.save(format!("output_tiles/{i}_{j}.png")).unwrap();
        }
    });

    for j in 0..level.height_in_tiles() {
        for i in 0..level.width_in_tiles() {
            let img = level.read_tile(i as i64, j as i64)?;
            tx.send((i, j, img)).unwrap();
        }
    }

    drop(tx);

    handle.join().unwrap();

    Ok(())
}
