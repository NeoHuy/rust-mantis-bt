use super::*;

#[test]
fn test_render_charts_empty_and_populated_xss_safe() {
    let empty_bar = render_horizontal_bar_chart("Empty <Bar>", &[]);
    assert!(empty_bar.contains("No chart data available."));
    assert!(!empty_bar.contains("<Bar>"));

    let items = vec![
        ("Core <App> & API".to_string(), 6usize, "#2b7dbc"),
        ("Mobile".to_string(), 4usize, "#629b58"),
    ];
    let bar = render_horizontal_bar_chart("By Project", &items);
    assert!(bar.contains("<svg"));
    assert!(bar.contains("6 (60%)"));
    assert!(bar.contains("Core &lt;App&gt; &amp; API"));
    assert!(!bar.contains("<App>"));
    assert!(!bar.contains("NaN"));

    let donut = render_donut_chart("By Status", &items);
    assert!(donut.contains("<svg"));
    assert!(donut.contains("10"));
    assert!(donut.contains("Total Issues"));
    assert!(!donut.contains("NaN"));

    let empty_trend = render_issues_trend_svg("Trend", &[]);
    assert!(empty_trend.contains("No trend data available."));

    let trend = render_issues_trend_svg(
        "Trend",
        &[
            TrendPoint {
                label: "2026-11-01".to_string(),
                opened: 3,
                resolved: 1,
                cumulative_open: 5,
            },
            TrendPoint {
                label: "2026-11-10".to_string(),
                opened: 2,
                resolved: 4,
                cumulative_open: 3,
            },
        ],
    );
    assert!(trend.contains("<polyline"));
    assert!(trend.contains("Backlog Open: 5"));
    assert!(!trend.contains("NaN"));
}
