//! CLI tests for Level of Detail (`--lod`) flag integration.

use clap::Parser;
use ctxcut_cli::{Cli, Commands};

#[test]
fn test_cli_lod_arguments_parsing() {
    let cli0 = Cli::try_parse_from([
        "ctxcut",
        "slice",
        "Assets/Player.cs:TakeDamage",
        "--lod",
        "0",
    ])
    .expect("parse --lod 0");
    if let Some(Commands::Slice { lod, .. }) = cli0.command {
        assert_eq!(lod, Some("0".to_string()));
    } else {
        panic!("Expected Slice command");
    }

    let cli1 = Cli::try_parse_from([
        "ctxcut",
        "slice",
        "Assets/Player.cs:TakeDamage",
        "--lod",
        "lod1",
    ])
    .expect("parse --lod lod1");
    if let Some(Commands::Slice { lod, .. }) = cli1.command {
        assert_eq!(lod, Some("lod1".to_string()));
    } else {
        panic!("Expected Slice command");
    }

    let cli2 = Cli::try_parse_from([
        "ctxcut",
        "slice",
        "Assets/Player.cs:TakeDamage",
        "--lod",
        "2",
    ])
    .expect("parse --lod 2");
    if let Some(Commands::Slice { lod, .. }) = cli2.command {
        assert_eq!(lod, Some("2".to_string()));
    } else {
        panic!("Expected Slice command");
    }
}
