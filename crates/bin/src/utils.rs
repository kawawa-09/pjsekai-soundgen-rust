macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        format!("\x1b[38;2;{};{};{}m", $r, $g, $b)
    };
    ($hex:expr) => {
        format!("\x1b[38;2;{};{};{}m", $hex >> 16, $hex >> 8 & 0xff, $hex & 0xff)
    };
    () => {
        "\x1b[0m"
    };
}

pub(crate) use rgb;

#[cfg(test)]
mod tests {
    #[test]
    fn rgb_formats_rgb_components() {
        assert_eq!(rgb!(255, 0, 128), "\x1b[38;2;255;0;128m");
    }

    #[test]
    fn rgb_formats_hex_color() {
        assert_eq!(rgb!(0x00b5c9), "\x1b[38;2;0;181;201m");
        assert_eq!(rgb!(0xffffff), "\x1b[38;2;255;255;255m");
    }

    #[test]
    fn rgb_without_arguments_resets_color() {
        assert_eq!(rgb!(), "\x1b[0m");
    }
}
