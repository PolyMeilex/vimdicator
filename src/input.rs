use gtk::gdk;
use gtk::Inhibit;

use log::debug;

pub fn keyval_to_input_string(in_str: &str, in_state: gdk::ModifierType) -> String {
    let mut val = in_str;
    let mut state = in_state;
    let empty = in_str.is_empty();

    if !empty {
        debug!("keyval -> {}", in_str);
    }

    // CTRL-^ and CTRL-@ don't work in the normal way.
    if state.contains(gdk::ModifierType::CONTROL_MASK)
        && !state.contains(gdk::ModifierType::SHIFT_MASK)
        && !state.contains(gdk::ModifierType::ALT_MASK)
        && !state.contains(gdk::ModifierType::META_MASK)
    {
        if val == "6" {
            val = "^";
        } else if val == "2" {
            val = "@";
        }
    }

    let chars: Vec<char> = in_str.chars().collect();

    if chars.len() == 1 {
        let ch = chars[0];

        // Remove SHIFT
        if ch.is_ascii() && !ch.is_alphanumeric() {
            state.remove(gdk::ModifierType::SHIFT_MASK);
        }
    }

    if val == "<" {
        val = "lt";
    }

    let mut mod_chars = Vec::<&str>::with_capacity(3);
    if state.contains(gdk::ModifierType::SHIFT_MASK) {
        mod_chars.push("S");
    }
    if state.contains(gdk::ModifierType::CONTROL_MASK) {
        mod_chars.push("C");
    }
    if state.contains(gdk::ModifierType::ALT_MASK) || state.contains(gdk::ModifierType::META_MASK) {
        mod_chars.push("A");
    }

    let sep = if empty { "" } else { "-" };
    let input = [mod_chars.as_slice(), &[val]].concat().join(sep);

    if !empty && input.chars().count() > 1 {
        format!("<{input}>")
    } else {
        input
    }
}

fn map_keyval(v: &str) -> Option<&'static str> {
    let v = match v {
        "F1" => "F1",
        "F2" => "F2",
        "F3" => "F3",
        "F4" => "F4",
        "F5" => "F5",
        "F6" => "F6",
        "F7" => "F7",
        "F8" => "F8",
        "F9" => "F9",
        "F10" => "F10",
        "F11" => "F11",
        "F12" => "F12",

        "Left" => "Left",
        "Right" => "Right",
        "Up" => "Up",
        "Down" => "Down",

        "Home" => "Home",
        "End" => "End",

        "BackSpace" => "BS",
        "Delete" => "Del",

        "Page_Up" => "PageUp",
        "Page_Down" => "PageDown",

        "Escape" => "Esc",
        "Tab" => "Tab",
        "ISO_Left_Tab" => "Tab",
        "Return" => "CR",
        "Enter" => "CR",
        "Insert" => "Insert",
        _ => return None,
    };

    Some(v)
}

pub fn convert_key(keyval: gdk::Key, modifiers: gdk::ModifierType) -> Option<String> {
    if let Some(ref keyval_name) = keyval.name() {
        if let Some(cnvt) = map_keyval(keyval_name.as_str()) {
            return Some(keyval_to_input_string(cnvt, modifiers));
        }
    }

    keyval
        .to_unicode()
        .map(|ch| keyval_to_input_string(&ch.to_string(), modifiers))
}

pub fn gtk_key_press_to_vim_input(
    keyval: gdk::Key,
    modifiers: gdk::ModifierType,
) -> (Inhibit, Option<String>) {
    if let Some(input) = convert_key(keyval, modifiers) {
        debug!("nvim_input -> {}", input);

        (Inhibit(true), Some(input))
    } else {
        (Inhibit(false), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyval_to_input_string() {
        macro_rules! test {
            ( $( $in_str:literal $( , $( $mod:ident )|* )? == $out_str:literal );*; ) => {
                let mut modifier;
                $(
                    modifier = gdk::ModifierType::empty() $( | $( gdk::ModifierType::$mod )|* )?;
                    assert_eq!(keyval_to_input_string($in_str, modifier), $out_str)
                );*
            }
        }

        test! {
            "a" == "a";
            "" == "";
            "6" == "6";
            "2" == "2";
            "<" == "<lt>";
            "", SHIFT_MASK == "S";
            "", SHIFT_MASK | CONTROL_MASK | ALT_MASK == "SCA";
            "a", SHIFT_MASK == "<S-a>";
            "a", SHIFT_MASK | CONTROL_MASK | ALT_MASK == "<S-C-A-a>";
            "6", CONTROL_MASK == "<C-^>";
            "6", CONTROL_MASK | META_MASK == "<C-A-6>";
            "2", CONTROL_MASK == "<C-@>";
            "2", CONTROL_MASK | ALT_MASK == "<C-A-2>";
            "j", SUPER_MASK == "j";
        }
    }
}
