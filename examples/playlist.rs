use anyhow::Result;
use bliss_audio::distance::{closest_to_first_song, dedup_playlist, euclidean_distance};
use bliss_audio::{library::analyze_paths_streaming, Song};
use clap::{App, Arg};
use glob::glob;
use mime_guess;
use serde_json;
use std::env;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

/* Analyzes a folder recursively, and make a playlist out of the file
 * provided by the user. */
// How to use: ./playlist [-o file.m3u] [-a analysis.json] <folder> <file to start the playlist from>
#[cfg(feature = "serde")]
fn main() -> Result<()> {
    let matches = App::new("playlist")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Polochon_street")
        .about("Analyze a folder and make a playlist from a target song")
        .arg(Arg::with_name("output-playlist").short("o").long("output-playlist")
            .value_name("PLAYLIST.M3U")
            .help("Outputs the playlist to a file.")
            .takes_value(true))
        .arg(Arg::with_name("analysis-file").short("a").long("analysis-file")
            .value_name("ANALYSIS.JSON")
            .help("Use the songs that have been analyzed in <analysis-file>, and appends newly analyzed songs to it. Defaults to /tmp/analysis.json.")
            .takes_value(true))
        .arg(Arg::with_name("FOLDER").help("Folders containing some songs.").required(true))
        .arg(Arg::with_name("FIRST-SONG").help("Song to start from (can be outside of FOLDER).").required(true))
        .get_matches();

    let folder = matches.value_of("FOLDER").unwrap();
    let file = fs::canonicalize(matches.value_of("FIRST-SONG").unwrap())?;
    let pattern = Path::new(folder).join("**").join("*");

    let mut songs: Vec<Song> = Vec::new();
    let analysis_path = matches
        .value_of("analysis-file")
        .unwrap_or("/tmp/analysis.json");
    let analysis_file = fs::File::open(analysis_path);
    if let Ok(f) = analysis_file {
        let reader = BufReader::new(f);
        songs = serde_json::from_reader(reader)?;
    }

    let analyzed_paths = songs
        .iter()
        .map(|s| s.path.to_owned())
        .collect::<Vec<PathBuf>>();

    let paths = glob(&pattern.to_string_lossy())?
        .map(|e| fs::canonicalize(e.unwrap()).unwrap())
        .filter(|e| match mime_guess::from_path(e).first() {
            Some(m) => m.type_() == "audio",
            None => false,
        })
        .map(|x| x.to_string_lossy().to_string())
        .collect::<Vec<String>>();

    let rx = analyze_paths_streaming(
        paths
            .iter()
            .filter(|p| !analyzed_paths.contains(&PathBuf::from(p)))
            .map(|p| p.to_owned())
            .collect(),
    )?;
    let first_song = Song::new(file)?;
    let mut analyzed_songs = vec![first_song.to_owned()];
    for (path, result) in rx.iter() {
        match result {
            Ok(song) => analyzed_songs.push(song),
            Err(e) => println!("error analyzing {}: {}", path, e),
        };
    }
    analyzed_songs.extend_from_slice(&songs);
    let serialized = serde_json::to_string(&analyzed_songs).unwrap();
    let mut songs_to_chose_from = analyzed_songs
        .into_iter()
        .filter(|x| x == &first_song || paths.contains(&x.path.to_string_lossy().to_string()))
        .collect();
    closest_to_first_song(&first_song, &mut songs_to_chose_from, euclidean_distance);
    dedup_playlist(&mut songs_to_chose_from, None);

    fs::write(analysis_path, serialized)?;
    let playlist = songs_to_chose_from
        .iter()
        .map(|s| s.path.to_string_lossy().to_string())
        .collect::<Vec<String>>()
        .join("\n");
    if let Some(m) = matches.value_of("output-playlist") {
        fs::write(m, playlist)?;
    } else {
        println!("{}", playlist);
    }
    Ok(())
}

#[cfg(not(feature = "serde"))]
fn main() {
    println!("You need the serde feature enabled to run this file.");
}
