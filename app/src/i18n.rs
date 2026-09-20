#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Locale {
    ZhCn,
    EnUs,
}

impl Locale {
    pub(crate) const fn language_tag(self) -> &'static str {
        match self {
            Self::ZhCn => "zh-CN",
            Self::EnUs => "en-US",
        }
    }
}

pub(crate) struct Messages {
    pub(crate) app_name: &'static str,
    pub(crate) home: &'static str,
    pub(crate) about: &'static str,
    pub(crate) language: &'static str,
    pub(crate) headline: &'static str,
    pub(crate) introduction: &'static str,
    pub(crate) start_hint: &'static str,
    pub(crate) about_title: &'static str,
    pub(crate) about_body: &'static str,
    pub(crate) stage_label: &'static str,
}

pub(crate) const fn messages(locale: Locale) -> &'static Messages {
    match locale {
        Locale::ZhCn => &ZH_CN,
        Locale::EnUs => &EN_US,
    }
}

const ZH_CN: Messages = Messages {
    app_name: "Zeta Practice",
    home: "首页",
    about: "关于",
    language: "界面语言",
    headline: "专注练习，持续进步",
    introduction: "这里将提供可重复、可测量的打字练习。",
    start_hint: "应用基础已就绪；打字功能将在后续 Stage 中加入。",
    about_title: "关于 Zeta Practice",
    about_body: "这是一个 Web-first、多用户能力练习平台。",
    stage_label: "Phase 1 · Stage 1.1 应用基础",
};

const EN_US: Messages = Messages {
    app_name: "Zeta Practice",
    home: "Home",
    about: "About",
    language: "Interface language",
    headline: "Focused practice, steady progress",
    introduction: "This space will provide repeatable, measurable typing practice.",
    start_hint: "The application foundation is ready; typing arrives in later Stages.",
    about_title: "About Zeta Practice",
    about_body: "A Web-first, multi-user platform for focused skill practice.",
    stage_label: "Phase 1 · Stage 1.1 application foundation",
};

#[cfg(test)]
mod tests {
    use super::{Locale, messages};

    #[test]
    fn both_locales_have_distinct_navigation_text() {
        assert_eq!(Locale::ZhCn.language_tag(), "zh-CN");
        assert_eq!(Locale::EnUs.language_tag(), "en-US");
        assert_eq!(messages(Locale::ZhCn).home, "首页");
        assert_eq!(messages(Locale::EnUs).home, "Home");
    }
}
