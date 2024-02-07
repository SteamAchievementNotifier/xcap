use image::RgbaImage;

fn main() {
    let dir_content = fs_extra::dir::get_dir_content("./examples").unwrap();
    for file in dir_content.files {
        if !file.ends_with(".txt") {
            continue;
        }

        let content = fs_extra::file::read_to_string(&file).unwrap();
        let buffer: Vec<u8> = content
            .split(",")
            .filter_map(|i| i.parse::<u8>().ok())
            .collect();

        let size: Vec<&str> = file
            .split("-")
            .last()
            .unwrap()
            .split(".txt")
            .next()
            .unwrap()
            .split("x")
            .collect();

        let width = size[0].parse::<u32>().unwrap();
        let height = size[1].parse::<u32>().unwrap();

        RgbaImage::from_raw(width, height, buffer)
            .unwrap()
            .save(format!("{}.png", file))
            .unwrap();
    }
}
