use super::*;
use crate::decode::tests::{fixture, temp_dir};
use crate::model::*;

fn monitor(id: &str, name: &str, x: f64) -> MonitorInfo {
    MonitorInfo {
        id: id.into(),
        gdi_name: String::new(),
        name: name.into(),
        px: Rect {
            x,
            y: 0.0,
            w: 64.0,
            h: 32.0,
        },
        scale: 1.0,
        primary: false,
        edid_mm: Some(Size { w: 100.0, h: 50.0 }),
        hdr: HdrState::default(),
    }
}

/// Two 64x32 monitors side by side; listed right-first to check {index} follows the desk.
fn job(
    image: &str,
    folder: &Path,
    format: ExportFormat,
    span: bool,
) -> (ExportJob, Vec<MonitorInfo>) {
    let mons = vec![
        monitor("R", "Right?", 64.0),
        monitor("L", "Left Mon: 1", 0.0),
    ];
    let placed = geometry::default_placement(&mons, crate::model::Align::Center, 0.0);
    let prefs = Prefs {
        format,
        folder: folder.to_string_lossy().into_owned(),
        template: "{image}_{monitor}_{index}_{w}x{h}".into(),
        span,
        ..Prefs::default()
    };
    let job = ExportJob {
        image: fixture(image),
        monitors: placed,
        framing: Framing::default(),
        prefs,
    };
    (job, mons)
}

#[test]
fn exports_named_tiles_and_a_span_image() {
    let dir = temp_dir("export-png");
    let (job, mons) = job("sdr8.png", &dir, ExportFormat::Png, true);
    let res = export(&job, &mons, 203.0).unwrap();
    let names: Vec<String> = res
        .files
        .iter()
        .map(|f| {
            Path::new(&f.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        names,
        ["sdr8_Left-Mon-1_1_64x32.png", "sdr8_Right_2_64x32.png"]
    );
    assert_eq!(res.files[0].monitor_id, "L");
    for f in &res.files {
        assert_eq!(image::image_dimensions(&f.path).unwrap(), (64, 32));
    }
    let span = res.span_file.unwrap();
    assert!(span.ends_with("sdr8_span_0_128x32.png"), "{span}");
    // The two tiles cover the image's middle band: left tile starts at the ramp's start.
    let img = image::open(&span).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (128, 32));
    assert!(img.get_pixel(0, 0)[0] < 8 && img.get_pixel(127, 0)[0] > 247);
    assert_eq!(res.folder, dir.to_string_lossy());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jpg_export_and_jxr_refused_for_sdr() {
    let dir = temp_dir("export-jpg");
    let (job_jpg, mons) = job("sdr16.tif", &dir, ExportFormat::Jpg { quality: 80 }, false);
    let res = export(&job_jpg, &mons, 203.0).unwrap();
    assert!(res.files.iter().all(|f| f.path.ends_with(".jpg")) && res.span_file.is_none());
    let (job_jxr, _) = job("sdr8.png", &dir, ExportFormat::Jxr, false);
    assert!(export(&job_jxr, &mons, 203.0).unwrap_err().contains("SDR"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jxr_round_trips_scrgb_through_wic() {
    let dir = temp_dir("export-jxr");
    let (job, mons) = job("hdr_pq16_bt2020.jxl", &dir, ExportFormat::Jxr, false);
    // SDR white 203 nits: a 1000-nit pixel becomes scRGB 12.5, above what u16 formats could hold.
    let res = export(&job, &mons, 203.0).unwrap();
    let right = &res.files[1].path;
    let (w, h, px, hdr) = wic::read(right).unwrap();
    assert_eq!((w, h, hdr), (64, 32, true));
    let crate::source::Pixels::F16(px) = px else {
        panic!("not f16")
    };
    // Right tile covers source x 64..128; its top-right pixel sits near the 1000-nit ramp end.
    let top_right = px[(63 * 3) as usize].to_f32();
    assert!(top_right > 11.0 && top_right < 13.0, "{top_right}");
    // Overwriting the same file works.
    export(&job, &mons, 203.0).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn names_are_sanitised_and_deduplicated() {
    assert_eq!(
        file_name(
            "{image}_{monitor}_{w}x{h}",
            "pic",
            "BenQ EW3270U",
            1,
            3840,
            2160,
            "png"
        ),
        "pic_BenQ-EW3270U_3840x2160.png"
    );
    assert_eq!(
        file_name("{monitor}", "pic", " a<b>:c\"d/e\\f|g?h*i ", 1, 1, 1, "jpg"),
        "abcdefghi.jpg"
    );
    assert_eq!(
        file_name("{image}", "My pic 6000 x 4000", "x", 1, 1, 1, "png"),
        "My-pic-6000-x-4000.png"
    );
    assert_eq!(file_name("{index}. ", "pic", "x", 2, 1, 1, "png"), "2.png");
    assert_eq!(
        file_name("???", "pic", "x", 2, 1, 1, "png"),
        "wallpaper.png"
    );
    let mut used = HashSet::new();
    assert_eq!(unique("a.png".into(), &mut used, 1), "a.png");
    assert_eq!(unique("A.png".into(), &mut used, 2), "A-2.png");
    // The source photo is a-2.png: the second tile skips that name instead of overwriting it.
    let mut used = HashSet::from(["a-2.png".to_string()]);
    assert_eq!(unique("a.png".into(), &mut used, 1), "a.png");
    assert_eq!(unique("a.png".into(), &mut used, 2), "a-3.png");
    assert_eq!(parse_fill("#ffffff"), [1.0; 3]);
    assert_eq!(parse_fill("nope"), [0.0; 3]);
}

#[test]
fn never_overwrites_the_source_photo() {
    let dir = temp_dir("export-self");
    let src = dir.join("sdr8.png");
    std::fs::copy(fixture("sdr8.png"), &src).unwrap();
    let (mut job, mons) = job("sdr8.png", &dir, ExportFormat::Png, false);
    job.image = src.to_string_lossy().into_owned();
    job.prefs.template = "{image}".into();
    let res = export(&job, &mons, 203.0).unwrap();
    let names: Vec<_> = res
        .files
        .iter()
        .map(|f| f.path.rsplit('\\').next().unwrap().to_string())
        .collect();
    assert_eq!(names, ["sdr8-1.png", "sdr8-2.png"]);
    assert_eq!(image::image_dimensions(&src).unwrap(), (128, 64));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn default_folders_keep_photos_with_the_same_name_apart() {
    let root = temp_dir("export-default");
    let (a, b) = (
        root.join("a").join("pic.jpg"),
        root.join("b").join("pic.jpg"),
    );
    let fa = default_folder(&root, "pic", &a).unwrap();
    assert_eq!(fa, root.join("pic"));
    assert_eq!(
        std::fs::read_to_string(fa.join(".source")).unwrap(),
        a.to_string_lossy()
    );
    assert_eq!(
        default_folder(&root, "pic", &b).unwrap(),
        root.join("pic (2)")
    );
    // The same photo again (any case) goes back to its own folder.
    let upper = PathBuf::from(a.to_string_lossy().to_uppercase());
    assert_eq!(default_folder(&root, "pic", &upper).unwrap(), fa);
    assert_eq!(
        default_folder(&root, "pic", &b).unwrap(),
        root.join("pic (2)")
    );
    // A folder from before markers existed is taken over by the first photo that uses it.
    std::fs::create_dir_all(root.join("old")).unwrap();
    assert_eq!(default_folder(&root, "old", &a).unwrap(), root.join("old"));
    let _ = std::fs::remove_dir_all(&root);
}
