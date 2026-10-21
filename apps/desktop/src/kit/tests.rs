//! The kit driven headless (egui_kittest, no GPU): clicks, keys and what they change.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use super::*;
use crate::theme;

/// A harness whose first frame installs the fonts and the theme (the faces must exist
/// before a widget asks for them), then runs `app` every frame.
fn harness<S: 'static>(
    state: S,
    mut app: impl FnMut(&mut egui::Ui, &mut S) + 'static,
) -> Harness<'static, (bool, S)> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 400.0))
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(
            move |ui, (ready, s): &mut (bool, S)| {
                if !*ready {
                    ui.ctx().set_fonts(theme::fonts::definitions());
                    theme::apply(ui.ctx(), Tokens::default());
                    install(ui.ctx());
                    *ready = true;
                    return;
                }
                app(ui, s);
            },
            (false, state),
        );
    h.run_steps(2);
    h
}

#[test]
fn switch_flips_on_click() {
    let mut h = harness(false, |ui, on| {
        ui.add(Switch::new(on, "Auto-finish"));
    });
    h.get_by_label("Auto-finish").click();
    h.run_steps(2);
    assert!(h.state().1);
}

#[test]
fn search_clear_empties_the_field() {
    let mut h = harness(String::from("chernarus"), |ui, s| {
        ui.add(Input::new(s).search().clear_label("Clear search"));
    });
    h.get_by_label("Clear search").click();
    h.run_steps(2);
    assert!(h.state().1.is_empty());
}

#[test]
fn segmented_picks_and_reports() {
    let mut h = harness((0u8, 0u32), |ui, (v, changes)| {
        let segs = [
            Segment::new(0u8, "SteamCMD"),
            Segment::new(1, "Steam client"),
        ];
        if ui.add(Segmented::new(v, &segs)).changed() {
            *changes += 1;
        }
    });
    h.get_by_label("Steam client").click();
    h.run_steps(2);
    assert_eq!(h.state().1, (1, 1));
}

#[test]
fn select_opens_and_picks() {
    let mut h = harness(Some("enoch"), |ui, v| {
        let opts = [
            SelectOption::new(Some("enoch"), "enoch"),
            SelectOption::new(Some("sakhal"), "sakhal"),
        ];
        ui.add(
            Select::new("map", v, &opts)
                .placeholder("All maps")
                .none(None)
                .label("Map"),
        );
    });
    h.get_by_label("Map").click();
    h.run_steps(3);
    h.get_by_label("sakhal").click();
    h.run_steps(3);
    assert_eq!(h.state().1, Some("sakhal"));
}

#[test]
fn virtual_list_lays_out_only_what_shows() {
    let mut h = harness(0usize, |ui, drawn| {
        *drawn = 0;
        VirtualList::new("rows", 100_000)
            .row_height(20.0)
            .overscan(2)
            .show(
                ui,
                |_| {},
                |ui, i| {
                    *drawn += 1;
                    ui.label(i.to_string());
                },
            );
    });
    h.run_steps(1);
    // 400 points of window, 20 a row: 20 rows and the overscan below.
    let drawn = h.state().1;
    assert!(drawn > 0 && drawn <= 24, "{drawn}");
}

#[test]
fn toasts_keep_the_newest_four_and_expire() {
    let mut h = harness((), |ui, _| {
        let _ = toast::host(ui.ctx(), "Dismiss");
    });
    for i in 0..6 {
        toast::say(&h.ctx, format!("toast {i}"), ToastTone::Ok);
    }
    h.run_steps(30);
    let live = h.ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<toast::Toasts>(toast::store_id())
            .list
            .len()
    });
    assert_eq!(live, 4);
    h.run_steps(60 * 5);
    let live = h.ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<toast::Toasts>(toast::store_id())
            .list
            .len()
    });
    assert_eq!(live, 0);
}

#[test]
fn checkbox_tick_and_confirm_answer() {
    let mut h = harness((false, true, false), |ui, (ticked, open, confirmed)| {
        ui.add(Checkbox::new(ticked).label("Select all"));
        if Confirm::new("c", "Delete?", "Delete route", "Cancel").show(ui.ctx(), open) {
            *confirmed = true;
        }
    });
    h.run_steps(20);
    h.get_by_label("Delete route").click();
    h.run_steps(20);
    let (_, open, confirmed) = h.state().1;
    assert!(confirmed && !open);
}
