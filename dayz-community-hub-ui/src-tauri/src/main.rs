// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;

fn main() {
    let args = dayz_community_hub_lib::CliArgs::parse();
    dayz_community_hub_lib::run(args);
}
