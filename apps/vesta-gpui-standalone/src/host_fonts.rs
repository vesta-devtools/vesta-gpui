//! System font selection belongs to the host.

use std::{fs, sync::Arc};
use vesta_gpui::{FontRole, TextInputContext};

pub(crate) fn register_host_fonts(context: &mut TextInputContext) {
    for path in [
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\segoeuii.ttf",
        r"C:\Windows\Fonts\segoeuiz.ttf",
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\seguiemj.ttf",
        r"C:\Windows\Fonts\seguisym.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ] {
        if let Ok(bytes) = fs::read(path) {
            context.register_font(Arc::new(bytes));
        }
    }
    let mut mono_registered = false;
    for path in [
        r"C:\Windows\Fonts\consola.ttf",
        r"C:\Windows\Fonts\consolab.ttf",
        r"C:\Windows\Fonts\consolai.ttf",
        r"C:\Windows\Fonts\consolaz.ttf",
    ] {
        if let Ok(bytes) = fs::read(path) {
            mono_registered |= context.register_font(Arc::new(bytes));
        }
    }
    if mono_registered {
        for role in [
            FontRole::Mono,
            FontRole::MonoBold,
            FontRole::MonoItalic,
            FontRole::MonoBoldItalic,
        ] {
            context.set_font_stack(role, "Consolas, Microsoft YaHei, Segoe UI Emoji, monospace");
        }
        context.set_font_stack(FontRole::MonoCjk, "Consolas, Microsoft YaHei, monospace");
    }
}
