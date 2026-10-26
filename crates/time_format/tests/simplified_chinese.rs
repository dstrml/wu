use time::macros::datetime;
use time_format::{TimestampFormat, format_date, format_date_medium, format_local_timestamp};

#[test]
fn simplified_chinese_timestamps() {
    i18n::set_language(i18n::Language::SimplifiedChinese);

    let reference = datetime!(1990-04-12 23:00 UTC);
    let relative =
        |timestamp| format_local_timestamp(timestamp, reference, TimestampFormat::Relative);
    assert_eq!(relative(reference), "刚刚");
    assert_eq!(relative(datetime!(1990-04-12 22:59 UTC)), "1 分钟前");
    assert_eq!(relative(datetime!(1990-04-12 22:15 UTC)), "45 分钟前");
    assert_eq!(relative(datetime!(1990-04-12 22:00 UTC)), "1 小时前");
    assert_eq!(relative(datetime!(1990-04-12 03:00 UTC)), "20 小时前");
    assert_eq!(relative(datetime!(1990-04-11 12:00 UTC)), "昨天");
    assert_eq!(relative(datetime!(1990-04-09 12:00 UTC)), "3 天前");
    assert_eq!(relative(datetime!(1990-04-05 12:00 UTC)), "1 周前");
    assert_eq!(relative(datetime!(1990-03-29 12:00 UTC)), "2 周前");
    assert_eq!(relative(datetime!(1990-03-12 12:00 UTC)), "4 周前");
    assert_eq!(relative(datetime!(1990-02-12 12:00 UTC)), "2 个月前");
    assert_eq!(relative(datetime!(1989-04-12 12:00 UTC)), "1 年前");
    assert_eq!(relative(datetime!(1989-03-12 12:00 UTC)), "1 年 1 个月前");
    assert_eq!(relative(datetime!(1988-06-12 12:00 UTC)), "1 年 10 个月前");
    assert_eq!(relative(datetime!(1988-03-12 12:00 UTC)), "2 年 1 个月前");
    assert_eq!(relative(datetime!(1987-05-12 12:00 UTC)), "2 年 11 个月前");
    assert_eq!(relative(datetime!(1988-04-12 12:00 UTC)), "2 年前");
    assert_eq!(relative(datetime!(1980-04-12 12:00 UTC)), "10 年前");

    let earlier_today = datetime!(1990-04-12 12:00 UTC);
    let yesterday = datetime!(1990-04-11 12:00 UTC);
    let older = datetime!(1990-04-10 12:00 UTC);
    assert_eq!(format_date(earlier_today, reference, true), "今天");
    assert_eq!(format_date(yesterday, reference, true), "昨天");
    assert_eq!(format_date_medium(yesterday, reference, true), "昨天");

    #[cfg(target_os = "macos")]
    {
        let absolute = |timestamp, format| format_local_timestamp(timestamp, reference, format);
        let enhanced_today = absolute(earlier_today, TimestampFormat::EnhancedAbsolute);
        assert!(enhanced_today.starts_with("今天 "), "{enhanced_today}");
        let enhanced_yesterday = absolute(yesterday, TimestampFormat::EnhancedAbsolute);
        assert!(
            enhanced_yesterday.starts_with("昨天 "),
            "{enhanced_yesterday}"
        );
        let enhanced_older = absolute(older, TimestampFormat::EnhancedAbsolute);
        assert!(enhanced_older.starts_with("1990/4/10 "), "{enhanced_older}");
        assert_eq!(
            absolute(older, TimestampFormat::MediumAbsolute),
            "1990年4月10日"
        );
    }
    #[cfg(not(target_os = "macos"))]
    assert!(!format_date(older, reference, false).is_empty());

    i18n::set_language(i18n::Language::English);
}
