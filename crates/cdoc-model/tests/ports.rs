//! The shared image port and its in-memory implementation.

use cdoc_model::{ContentId, ImageError, ImageLibrary, ImagePort};

#[test]
fn images_are_served_by_content_id() {
    let mut library = ImageLibrary::empty();
    library.add(ContentId::new("a"), "image/png", vec![1, 2, 3]);
    library.add(ContentId::new("b"), "image/jpeg", vec![9]);
    let a = library.image(&ContentId::new("a")).expect("held");
    assert_eq!((a.mime, a.bytes), ("image/png", &[1u8, 2, 3][..]));
    assert_eq!(
        library.image(&ContentId::new("b")).expect("held").mime,
        "image/jpeg"
    );
}

#[test]
fn adding_under_an_existing_id_replaces_the_image() {
    let mut library = ImageLibrary::empty();
    library.add(ContentId::new("a"), "image/png", vec![1]);
    library.add(ContentId::new("a"), "image/gif", vec![2]);
    let a = library.image(&ContentId::new("a")).expect("held");
    assert_eq!((a.mime, a.bytes), ("image/gif", &[2u8][..]));
}

#[test]
fn a_missing_image_names_its_id() {
    let library = ImageLibrary::empty();
    let err = library.image(&ContentId::new("nope")).expect_err("absent");
    assert_eq!(err, ImageError("nope".to_owned()));
    assert_eq!(err.to_string(), "no image with content id \"nope\"");
}
