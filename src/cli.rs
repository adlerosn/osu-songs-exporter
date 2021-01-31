use std::path::PathBuf;

#[derive(Debug, Clone, new)]
pub struct CliArguments {
    pub filename_template: String,
    pub osu_source: PathBuf,
    pub songs_destination: PathBuf,
    pub unicode_filename: bool,
    pub remove_missing_songs: bool,
}

pub fn get_arguments_parsed() -> CliArguments {
    let mut ca = CliArguments::new(
        "osu! - %a - %t #%i".to_string(),
        PathBuf::from(""),
        PathBuf::from(""),
        true,
        false,
    );
    {
        let mut parser = argparse::ArgumentParser::new();
        parser.set_description("Exports your Osu! songs library as a music folder");

        parser.refer(&mut ca.remove_missing_songs).add_option(
            &["-r", "--remove"],
            argparse::StoreTrue,
            "Removes songs from the destination folder that can't be found within Osu!",
        );
        parser.refer(&mut ca.unicode_filename).add_option(
            &["-a", "--ascii-filenames"],
            argparse::StoreFalse,
            "Use ASCII filenames for naming songs in the filesystem",
        );
        parser
            .refer(&mut ca.osu_source)
            .add_argument("osu_source", argparse::Store, "Your Osu! folder")
            .required();
        parser
            .refer(&mut ca.songs_destination)
            .add_argument(
                "songs_destination",
                argparse::Store,
                "Your song library folder (NOT Osu!'s)",
            )
            .required();
        parser.refer(&mut ca.filename_template).add_option(
            &["-t", "--template"],
            argparse::Store,
            "\"osu! - %a - %t #%i\"",
        );
        parser.parse_args_or_exit();
    }
    ca
}
