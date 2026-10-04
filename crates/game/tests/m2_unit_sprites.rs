//! M2 placeholder unit sprites: the committed sheets are the generator's output, they pass the
//! PNG subset the renderer is restricted to (docs/DECISIONS.md D16, D20), and frame i of a sheet
//! points in the screen direction of `sim::Direction::ALL[i]`.

#[path = "../examples/gen_unit_sprites.rs"]
#[allow(dead_code)]
mod gen_unit_sprites;

#[path = "../src/tileset_image.rs"]
#[allow(dead_code)]
mod tileset_image;

use gen_unit_sprites::{FACING_DELTAS, FACINGS, FRAME, GROUND, SPRITES, sheet_pixels};
use sim::Direction;
use tileset_image::{TilesetImageErrorKind, check_png_file};

#[test]
fn committed_sprite_sheets_match_generator_output() {
    let hint = "run `cargo run -p game --example gen_unit_sprites`";
    for kind in &SPRITES {
        let path = gen_unit_sprites::output_dir().join(format!("{}.png", kind.name));
        let committed = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(
            committed == gen_unit_sprites::sheet_png(kind),
            "{} differs; {hint}",
            path.display()
        );
    }
}

#[test]
fn sprite_sheets_pass_the_png_subset_with_their_declared_size() {
    for kind in &SPRITES {
        let path = gen_unit_sprites::output_dir().join(format!("{}.png", kind.name));
        let bytes =
            check_png_file(&path, FRAME * FACINGS, FRAME).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bytes, std::fs::read(&path).expect("sheet"));
        // Any other declared size is rejected.
        let error = check_png_file(&path, FRAME * FACINGS, FRAME + 1).expect_err("size");
        assert!(matches!(
            error.kind,
            TilesetImageErrorKind::SizeMismatch { .. }
        ));
    }
}

#[test]
fn the_facing_order_is_the_order_of_sim_directions() {
    assert_eq!(FACING_DELTAS.len(), Direction::ALL.len());
    for (index, direction) in Direction::ALL.into_iter().enumerate() {
        assert_eq!(FACING_DELTAS[index], direction.delta());
        assert_eq!(usize::from(direction.index()), index);
    }
}

#[test]
fn every_frame_points_in_the_screen_direction_of_its_facing() {
    // The centroid of a frame's front pixels (the darker front of the hull and the barrel)
    // lies, from the ground point, in the direction of equation 7 applied to the facing's cell
    // offset: screen x grows with dx − dy, and screen y (downwards) with dx + dy.
    const FRONT: [[u8; 3]; 2] = [[140, 140, 140], [70, 70, 70]];
    for kind in &SPRITES {
        let (width, _, pixels) = sheet_pixels(kind);
        for (frame, (dx, dy)) in FACING_DELTAS.into_iter().enumerate() {
            let (mut sum_x, mut sum_y, mut count) = (0.0_f64, 0.0_f64, 0.0_f64);
            for y in 0..FRAME {
                for x in 0..FRAME {
                    let at = (((y * width) + frame as u32 * FRAME + x) * 4) as usize;
                    if FRONT.iter().any(|colour| pixels[at..at + 3] == colour[..]) {
                        sum_x += f64::from(x) - f64::from(GROUND.0);
                        sum_y += f64::from(y) - f64::from(GROUND.1);
                        count += 1.0;
                    }
                }
            }
            assert!(count > 0.0, "{} frame {frame}: no front pixels", kind.name);
            let centroid = (sum_x / count, sum_y / count);
            let expected = (f64::from(dx - dy) * 32.0, f64::from(dx + dy) * 16.0);
            let cosine = (centroid.0 * expected.0 + centroid.1 * expected.1)
                / (centroid.0.hypot(centroid.1) * expected.0.hypot(expected.1));
            assert!(
                cosine > 0.7,
                "{} frame {frame}: front at {centroid:?}, facing {expected:?}",
                kind.name
            );
        }
    }
}

#[test]
fn a_named_pipe_is_rejected_without_blocking() {
    // M1 review 3, P2-1: opening a named pipe blocks until a writer appears. The image check
    // rejects anything that is not a regular file before opening it.
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("m2_unit_sprites");
    std::fs::create_dir_all(&dir).expect("temporary directory");
    let fifo = dir.join("pipe.png");
    let _ = std::fs::remove_file(&fifo);
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
    let (sender, receiver) = std::sync::mpsc::channel();
    let path = fifo.clone();
    std::thread::spawn(move || {
        let _ = sender.send(check_png_file(&path, 512, 64));
    });
    let result = receiver
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the check returned instead of blocking");
    let error = result.expect_err("a pipe is not an image");
    assert_eq!(
        (error.field.as_str(), &error.kind),
        (
            "file",
            &TilesetImageErrorKind::Read("not a regular file".into())
        )
    );
    std::fs::remove_file(&fifo).expect("remove the pipe");
}
