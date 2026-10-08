use super::*;
use crate::forms::parse;
use crate::forms::Button;

fn data(form: &Form, reply: FormReply) -> String {
    response(form, &reply).unwrap().data.unwrap()
}

#[test]
fn simple_form_with_legacy_buttons() {
    let json = r#"{"type":"form","title":"§lShop","content":"Pick one","buttons":[{"text":"Blocks","image":{"type":"path","data":"textures/blocks/dirt"}},{"text":"Tools"}]}"#;
    let form = parse::form(7, json).unwrap();
    let FormKind::Simple { buttons, .. } = &form.kind else { panic!() };
    assert_eq!(buttons[0], Button { text: "Blocks".into(), image: Some("textures/blocks/dirt".into()) });
    assert_eq!(buttons[1].image, None);
    assert_eq!(data(&form, FormReply::Button(1)), "1\n");
    assert!(response(&form, &FormReply::Button(2)).is_err());
    assert!(response(&form, &FormReply::Modal(true)).is_err());
}

#[test]
fn simple_form_with_mixed_elements_indexes_buttons_only() {
    let json = r#"{"type":"form","title":{"rawtext":[{"text":"Menu"}]},"content":"","elements":[{"type":"header","text":"Top"},{"type":"button","text":"A"},{"type":"divider","text":""},{"type":"button","text":"B"}]}"#;
    let form = parse::form(1, json).unwrap();
    assert_eq!(form.title, "Menu");
    let FormKind::Simple { buttons, elements, positions, .. } = &form.kind else { panic!() };
    assert_eq!(buttons.iter().map(|b| b.text.as_str()).collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(elements, &[Element::Header { text: "Top".into() }, Element::Divider]);
    assert_eq!(positions, &[0, 1], "the header before A, the divider between A and B");
    assert_eq!(data(&form, FormReply::Button(1)), "1\n");
}

#[test]
fn modal_answers_true_or_false() {
    let form = parse::form(2, r#"{"type":"modal","title":"Sure?","content":"Really","button1":"Yes","button2":"No"}"#).unwrap();
    assert_eq!(form.kind, FormKind::Modal { content: "Really".into(), yes: "Yes".into(), no: "No".into() });
    assert_eq!(data(&form, FormReply::Modal(true)), "true\n");
    assert_eq!(data(&form, FormReply::Modal(false)), "false\n");
}

#[test]
fn custom_form_fills_null_slots_and_checks_values() {
    let json = r#"{"type":"custom_form","title":"Pay","content":[
        {"type":"label","text":"Send money"},
        {"type":"input","text":"Player","placeholder":"name","default":""},
        {"type":"toggle","text":"Anonymous","default":true},
        {"type":"slider","text":"Amount","min":1,"max":100,"step":1,"default":5},
        {"type":"dropdown","text":"Currency","options":["coins","gems"],"default":1},
        {"type":"step_slider","text":"Speed","steps":["slow","fast"]}
    ]}"#;
    let form = parse::form(3, json).unwrap();
    let defaults = form.defaults();
    assert_eq!(
        defaults,
        [FormValue::Text(String::new()), FormValue::Toggle(true), FormValue::Slider(5.0), FormValue::Choice(1), FormValue::Choice(0)]
    );
    // Vanilla's reply to the same form minus the step slider (2026-10-02 capture): `[null,"",true,5,1]`.
    assert_eq!(data(&form, FormReply::Custom(defaults.clone())), "[null,\"\",true,5,1,0]\n");
    assert_eq!(form.inputs(), 5);
    let mut half = defaults.clone();
    half[2] = FormValue::Slider(2.5);
    assert_eq!(data(&form, FormReply::Custom(half)), "[null,\"\",true,2.5,1,0]\n");

    let mut wrong = defaults.clone();
    wrong[3] = FormValue::Choice(2);
    assert!(response(&form, &FormReply::Custom(wrong)).is_err());
    let mut wrong = defaults.clone();
    wrong[2] = FormValue::Slider(101.0);
    assert!(response(&form, &FormReply::Custom(wrong)).is_err());
    assert!(response(&form, &FormReply::Custom(defaults[..4].to_vec())).is_err());
}

#[test]
fn close_sends_reason_without_data() {
    let p = cancel(9);
    assert_eq!((p.form_id, p.has_response_data, p.data, p.has_cancel_reason), (9, false, None, true));
    assert_eq!(p.content.unwrap().cancel_reason, ModalFormResponseContentCancelReason::Closed);
}

#[test]
fn unknown_or_broken_forms_are_skipped() {
    assert!(parse::form(1, "{").is_none());
    assert!(parse::form(1, r#"{"type":"data_driven"}"#).is_none());
}
