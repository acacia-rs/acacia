use std::time::Instant;

use super::*;
use crate::input::Mods;
use crate::widget::TextEdit;
use crate::widget::test_theme::theme;

const SIZE: [f32; 2] = [320.0, 240.0];

fn action() -> Spec {
    Spec::Action {
        content: "Pick one".into(),
        items: vec![
            Widget::Button { text: "First".into(), image: None },
            Widget::Text { text: "Between".into() },
            Widget::Button { text: "Second".into(), image: None },
        ],
    }
}

fn click(view: &mut FormView, theme: &Theme, at: [f32; 2]) -> Option<Outcome> {
    view.handle(&Input::Move(at), theme);
    view.handle(&Input::Press(at), theme);
    view.handle(&Input::Release(at), theme)
}

#[test]
fn bedrock_action_form_sits_in_the_dialog() {
    let theme = theme(false);
    let mut view = FormView::new("Title", action(), &theme, SIZE);
    // The dialog is centred: (47, 20); the viewport starts at (57, 43).
    assert_eq!(view.chrome, Chrome::Bedrock([47.0, 20.0]));
    assert_eq!(view.panel.rect(0), [59.0, 45.0, 253.0, 55.0], "body at (2, 2), one line");
    assert_eq!(view.panel.rect(1), [61.0, 59.0, 251.0, 91.0], "4 below the body, 190 wide, 32 tall");
    assert_eq!(view.panel.rect(2)[1], 91.0 + 6.0, "a label's text 6 into its row");
    let second = view.panel.rect(3);
    assert_eq!(second[1], 91.0 + 10.0 + 11.0);
    assert_eq!(click(&mut view, &theme, [second[0] + 5.0, second[1] + 5.0]), Some(Outcome::Button(1)), "counted among buttons only");
    assert_eq!(click(&mut view, &theme, [47.0 + 210.0, 20.0 + 10.0]), Some(Outcome::Close), "the X");
}

#[test]
fn modal_buttons_stack_in_bedrock_and_sit_in_javas_footer() {
    let spec = Spec::Modal { content: "Sure?".into(), yes: "Yes".into(), no: "No".into() };
    let theme_b = theme(false);
    let mut view = FormView::new("", spec.clone(), &theme_b, SIZE);
    let [yes, no] = [view.panel.rect(1), view.panel.rect(2)];
    assert_eq!(no[1], yes[3], "button1 above button2, no gap");
    assert_eq!(click(&mut view, &theme_b, [no[0] + 3.0, no[1] + 3.0]), Some(Outcome::Modal(false)));

    let theme_j = theme(true);
    let mut view = FormView::new("", spec, &theme_j, SIZE);
    let [yes, no] = [view.panel.rect(1), view.panel.rect(2)];
    assert_eq!(yes, [6.0, 214.0, 156.0, 234.0], "150 wide, 8 apart, centred in the 33 px footer");
    assert_eq!(no[0], 164.0);
    assert_eq!(click(&mut view, &theme_j, [yes[0] + 3.0, yes[1] + 3.0]), Some(Outcome::Modal(true)));
}

#[test]
fn custom_form_submits_values_in_order_and_esc_closes() {
    for java in [false, true] {
        let theme = theme(java);
        let elements = vec![
            Widget::Text { text: "Fill in".into() },
            Widget::Toggle { label: "On".into(), on: true },
            Widget::Input { label: "Name".into(), placeholder: "you".into(), edit: TextEdit::new("", 100) },
        ];
        let mut view = FormView::new("Form", Spec::Custom { elements, submit: SUBMIT.into() }, &theme, SIZE);
        let field = theme.widgets.skin().hit(theme.font.as_ref(), &view.panel.items[2].widget, view.panel.rect(2));
        click(&mut view, &theme, [field[0] + 2.0, field[1] + 2.0]);
        view.handle(&Input::Text("Ann".into()), &theme);
        view.resize(&theme, [400.0, 300.0]);
        let submit = view.panel.rect(3);
        let out = click(&mut view, &theme, [submit[0] + 3.0, submit[1] + 3.0]);
        assert_eq!(out, Some(Outcome::Submit(vec![Value::Toggle(true), Value::Text("Ann".into())])), "java look: {java}");
        assert_eq!(view.handle(&Input::Key(Key::Escape, Mods::default()), &theme), Some(Outcome::Close));
    }
}

#[test]
fn both_looks_draw_without_sprites() {
    for java in [false, true] {
        let theme = theme(java);
        let mut list = DrawList::new(2.0);
        FormView::new("Title", action(), &theme, SIZE).draw(&mut list, &theme, Instant::now());
        assert!(list.quads.len() > 20, "fills stand in for missing sprites, text draws");
    }
}
