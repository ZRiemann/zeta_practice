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

    pub(crate) fn from_language_tag(tag: &str) -> Self {
        if tag == "en-US" {
            Self::EnUs
        } else {
            Self::ZhCn
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
    pub(crate) login: &'static str,
    pub(crate) register: &'static str,
    pub(crate) logout: &'static str,
    pub(crate) username: &'static str,
    pub(crate) password: &'static str,
    pub(crate) welcome: &'static str,
    pub(crate) login_hint: &'static str,
    pub(crate) register_hint: &'static str,
    pub(crate) invalid_input: &'static str,
    pub(crate) username_taken: &'static str,
    pub(crate) invalid_credentials: &'static str,
    pub(crate) login_required: &'static str,
    pub(crate) request_failed: &'static str,
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
    stage_label: "Phase 1 · Stage 1.3 账户与登录",
    login: "登录",
    register: "注册",
    logout: "注销",
    username: "用户名",
    password: "密码",
    welcome: "欢迎，",
    login_hint: "使用用户名和密码登录。",
    register_hint: "创建账户后即可登录。用户名需为 3–32 位英文字母、数字、下划线或连字符；密码需为 12–128 字节。",
    invalid_input: "请检查用户名、密码和语言设置。",
    username_taken: "用户名已被使用。",
    invalid_credentials: "用户名或密码错误。",
    login_required: "请先登录。",
    request_failed: "操作失败，请稍后再试。",
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
    stage_label: "Phase 1 · Stage 1.3 accounts and login",
    login: "Log in",
    register: "Register",
    logout: "Log out",
    username: "Username",
    password: "Password",
    welcome: "Welcome, ",
    login_hint: "Log in with your username and password.",
    register_hint: "Create an account to sign in. Usernames use 3–32 letters, digits, underscores or hyphens; passwords use 12–128 bytes.",
    invalid_input: "Check the username, password, and language.",
    username_taken: "That username is taken.",
    invalid_credentials: "Incorrect username or password.",
    login_required: "Please log in first.",
    request_failed: "The request failed. Please try again.",
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
        assert_eq!(messages(Locale::ZhCn).login, "登录");
        assert_eq!(messages(Locale::EnUs).register, "Register");
    }
}
