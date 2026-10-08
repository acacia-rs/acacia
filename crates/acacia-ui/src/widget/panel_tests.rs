use super::*;
use crate::input::{Input, Key, Mods};
use crate::widget::test_theme::theme;
use crate::widget::{TextEdit, Value};

/// A Bedrock-skinned column: toggle, button, slider, input, then a dropdown out of view.
fn panel() -> Panel {
    let rows = [
        (Widget::Toggle { label: "T".into(), on: false }, 20.0),
        (Widget::Button { text: "B".into(), image: None }, 32.0),
        (Widget::Slider { label: "S".into(), min: 0.0, max: 10.0, step: 1.0, value: 0.0 }, 32.0),
        (Widget::Input { label: "I".into(), placeholder: String::new(), edit: TextEdit::new("", 10) }, 46.0),
        (Widget::Dropdown { label: "D".into(), options: vec!["a".into(), "b".into(), "c".into()], index: 0 }, 46.0),
    ];
    let mut y = 0.0;
    let items = rows
        .into_iter()
        .map(|(widget, h)| {
            let rect = [0.0, y, 194.0, y + h];
            y += h;
            Placed { widget, rect, scrolls: true }
        })
        .collect();
    Panel::new(items, [0.0, 0.0, 200.0, 100.0], 0.0)
}

fn send(panel: &mut Panel, theme: &crate::theme::Theme, inputs: &[Input]) -> Vec<Response> {
    inputs.iter().map(|i| panel.handle(i, theme.widgets.skin(), theme.font.as_ref())).collect()
}

#[test]
fn a_button_fires_on_release_over_it() {
    let theme = theme(false);
    let mut p = panel();
    let r = send(&mut p, &theme, &[Input::Press([5.0, 30.0]), Input::Release([5.0, 30.0])]);
    assert_eq!(r, [Response::Consumed, Response::Pressed(1)]);
    let r = send(&mut p, &theme, &[Input::Press([5.0, 30.0]), Input::Move([5.0, 2.0]), Input::Release([5.0, 2.0])]);
    assert_eq!(r[2], Response::Consumed, "released elsewhere: no press");
}

#[test]
fn clicks_set_toggles_and_sliders() {
    let theme = theme(false);
    let mut p = panel();
    send(&mut p, &theme, &[Input::Press([5.0, 5.0]), Input::Release([5.0, 5.0])]);
    // The slider's control is 12 below its row top; its handle runs from x 10 to 184.
    send(&mut p, &theme, &[Input::Press([10.0 + 174.0 * 0.62, 70.0]), Input::Release([0.0, 0.0])]);
    assert_eq!(p.values()[..2], [Value::Toggle(true), Value::Number(6.0)], "snapped to whole steps");
}

#[test]
fn typing_goes_to_the_focused_input_and_tab_scrolls_to_it() {
    let theme = theme(false);
    let mut p = panel();
    let tab = Input::Key(Key::Tab, Mods::default());
    send(&mut p, &theme, &[tab.clone(), tab.clone(), tab.clone(), tab.clone()]);
    send(&mut p, &theme, &[Input::Text("hi".into()), Input::Key(Key::Backspace, Mods::default()), Input::Text("ey".into())]);
    assert_eq!(p.values()[2], Value::Text("hey".into()));
    send(&mut p, &theme, &[tab]);
    assert_eq!(p.rect(4)[3], 100.0, "the dropdown scrolled into view");
}

#[test]
fn wheel_scrolls_within_the_content() {
    let theme = theme(false);
    let mut p = panel();
    assert_eq!(p.max_scroll(), 76.0);
    send(&mut p, &theme, &[Input::Wheel(-100.0)]);
    assert_eq!(p.rect(0)[1], -76.0);
    send(&mut p, &theme, &[Input::Wheel(100.0)]);
    assert_eq!(p.rect(0)[1], 0.0);
}

#[test]
fn bedrock_dropdowns_open_a_list_java_cycles() {
    let theme_b = theme(false);
    let mut p = panel();
    send(&mut p, &theme_b, &[Input::Wheel(-100.0)]);
    let control = p.rect(4)[1] + 12.0 + 5.0;
    send(&mut p, &theme_b, &[Input::Press([20.0, control]), Input::Release([20.0, control])]);
    // The list opens below the control; its rows are 17 apart from 3 below its top.
    let second = p.rect(4)[3] - 4.0 + 3.0 + 17.0 + 5.0;
    send(&mut p, &theme_b, &[Input::Press([20.0, second])]);
    assert_eq!(p.values()[3], Value::Choice(1));

    let theme_j = theme(true);
    let mut p = panel();
    send(&mut p, &theme_j, &[Input::Wheel(-100.0)]);
    let row = p.rect(4);
    send(&mut p, &theme_j, &[Input::Press([20.0, row[1] + 5.0]), Input::Press([20.0, row[1] + 5.0])]);
    assert_eq!(p.values()[3], Value::Choice(2));
}
