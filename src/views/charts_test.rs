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

#[test]
fn test_render_charts_top_n_sorted_with_other_and_full_total() {
    // 15 categories of equal descending sizes (16 down to 2); total = 135.
    let items: Vec<(String, usize, &str)> = (0..15)
        .map(|i| (format!("Cat{:02}", i), 16 - i, "#2b7dbc"))
        .collect();
    let total: usize = items.iter().map(|(_, c, _)| *c).sum();
    assert_eq!(total, 135);

    // Unsorted input still renders highest count first and aggregates tail into "Other".
    let mut unsorted = items.clone();
    unsorted.reverse();

    let bar = render_horizontal_bar_chart("Top Bar", &unsorted);
    let idx_top = bar.find("Cat00").unwrap();
    let idx_second = bar.find("Cat01").unwrap();
    let idx_other = bar.find("Other").unwrap();
    assert!(idx_top < idx_second);
    assert!(idx_second < idx_other);
    assert!(!bar.contains("Cat11"));
    // Tail of 4 items (5+4+3+2 = 14) -> 14*100/135 = 10%
    assert!(bar.contains("14 (10%)"));
    // Top item (16) uses full total 135 -> 16*100/135 = 11%
    assert!(bar.contains("16 (11%)"));

    let donut = render_donut_chart("Top Donut", &unsorted);
    assert!(donut.contains(&format!(">{}</text>", total)));
    assert!(donut.contains("Other"));
    assert!(!donut.contains("Cat09"));

    // Real "Other" category in top items triggers "(combined)" suffix to avoid duplicate labels
    let mut with_real_other = items.clone();
    with_real_other[0].0 = "Other".to_string();
    let collision_bar = render_horizontal_bar_chart("Collision Bar", &with_real_other);
    assert!(collision_bar.contains(">Other<"));
    assert!(collision_bar.contains("Other (combined)"));
}

#[test]
fn test_trend_y_axis_ticks_are_unique_increasing_integers() {
    for max_val in [1usize, 2, 3, 4, 5, 9, 1000] {
        let svg = render_issues_trend_svg(
            "Trend",
            &[TrendPoint {
                label: "2026-11-01".to_string(),
                opened: max_val,
                resolved: 0,
                cumulative_open: max_val,
            }],
        );
        let ticks: Vec<usize> = svg
            .split("text-anchor=\"end\" font-size=\"10\" fill=\"#6b7280\">")
            .skip(1)
            .filter_map(|s| s.split('<').next()?.parse().ok())
            .collect();
        assert!(ticks.len() >= 2, "max_val={} ticks={:?}", max_val, ticks);
        assert_eq!(ticks[0], 0);
        assert_eq!(*ticks.last().unwrap(), max_val);
        assert!(
            ticks.windows(2).all(|w| w[0] < w[1]),
            "max_val={} ticks={:?}",
            max_val,
            ticks
        );
    }
}
