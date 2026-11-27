use crate::models::TrendPoint;
use crate::views::html_escape;

/// Server-side palette matching MantisBT UI tokens.
pub const CHART_PALETTE: [&str; 8] = [
    "#2b7dbc", "#629b58", "#f59942", "#d15b47", "#9585bf", "#3a87ad", "#82af6f", "#a0a0a0",
];

fn truncate_label(label: &str, max_chars: usize) -> String {
    let count = label.chars().count();
    if count <= max_chars {
        label.to_string()
    } else if max_chars > 3 {
        let prefix: String = label.chars().take(max_chars - 3).collect();
        format!("{}...", prefix)
    } else {
        label.chars().take(max_chars).collect()
    }
}

fn aggregate_chart_items<'a>(
    items: &'a [(String, usize, &'a str)],
    cap: usize,
) -> (Vec<(String, usize, &'a str)>, usize) {
    let mut sorted: Vec<(String, usize, &'a str)> = items
        .iter()
        .filter(|(_, cnt, _)| *cnt > 0)
        .map(|(lbl, cnt, col)| (lbl.clone(), *cnt, *col))
        .collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let total: usize = sorted.iter().map(|(_, cnt, _)| *cnt).sum();
    if sorted.len() > cap && cap > 1 {
        let keep = cap - 1;
        let other_sum: usize = sorted[keep..].iter().map(|(_, cnt, _)| *cnt).sum();
        sorted.truncate(keep);
        if other_sum > 0 {
            let other_label = if sorted.iter().any(|(lbl, _, _)| lbl == "Other") {
                "Other (combined)".to_string()
            } else {
                "Other".to_string()
            };
            sorted.push((other_label, other_sum, "#9ca3af"));
        }
    }
    (sorted, total)
}

/// Renders a responsive inline SVG horizontal bar chart (no external scripts/CDN).
pub fn render_horizontal_bar_chart(title: &str, items: &[(String, usize, &str)]) -> String {
    let (non_zero, total) = aggregate_chart_items(items, 12);
    let max_val = non_zero.iter().map(|(_, cnt, _)| *cnt).max().unwrap_or(0);

    if non_zero.is_empty() || max_val == 0 || total == 0 {
        return format!(
            r#"<div class="text-xs text-gray-500 text-center py-8" role="region" aria-label="{}">No chart data available.</div>"#,
            html_escape(title)
        );
    }

    let width = 560usize;
    let label_w = 155usize;
    let right_pad = 75usize;
    let bar_max_w = width - label_w - right_pad;
    let row_h = 26usize;
    let top_pad = 14usize;
    let height = top_pad + non_zero.len() * row_h + 10;

    let mut bars_svg = String::new();
    for (idx, (label, count, color)) in non_zero.iter().enumerate() {
        let y = top_pad + idx * row_h;
        let bar_w = (((*count as f64) / (max_val as f64)) * (bar_max_w as f64))
            .max(4.0)
            .round();
        let pct = (*count * 100).checked_div(total).unwrap_or(0);
        let short_label = truncate_label(label, 22);
        bars_svg.push_str(&format!(
            r##"<g class="chart-row">
                <title>{full_label}: {count} ({pct}%)</title>
                <text x="{lbl_x}" y="{text_y}" text-anchor="end" font-size="11" fill="#374151">{short_label}</text>
                <rect x="{bar_x}" y="{bar_y}" width="{bar_max_w}" height="16" rx="3" fill="#f3f4f6" />
                <rect x="{bar_x}" y="{bar_y}" width="{bar_w:.1}" height="16" rx="3" fill="{color}" />
                <text x="{val_x:.1}" y="{text_y}" font-size="11" font-weight="600" fill="#1f2937">{count} ({pct}%)</text>
            </g>"##,
            full_label = html_escape(label),
            short_label = html_escape(&short_label),
            count = count,
            pct = pct,
            lbl_x = label_w - 8,
            text_y = y + 12,
            bar_x = label_w,
            bar_y = y,
            bar_max_w = bar_max_w,
            bar_w = bar_w,
            color = html_escape(color),
            val_x = (label_w as f64) + bar_max_w as f64 + 6.0,
        ));
    }

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" class="w-full h-auto" role="img" aria-label="{title}">
            {bars_svg}
        </svg>"##,
        width = width,
        height = height,
        title = html_escape(title),
        bars_svg = bars_svg
    )
}

/// Renders an inline SVG donut chart + legend using circumference stroke dashes.
pub fn render_donut_chart(title: &str, items: &[(String, usize, &str)]) -> String {
    let (non_zero, total) = aggregate_chart_items(items, 10);

    if non_zero.is_empty() || total == 0 {
        return format!(
            r#"<div class="text-xs text-gray-500 text-center py-8" role="region" aria-label="{}">No chart data available.</div>"#,
            html_escape(title)
        );
    }

    let radius = 64.0f64;
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let cx = 96.0f64;
    let cy = 96.0f64;

    let mut slices_svg = String::new();
    let mut legend_svg = String::new();
    let mut offset = 0.0f64;

    for (idx, (label, count, color)) in non_zero.iter().enumerate() {
        let frac = (*count as f64) / (total as f64);
        let dash = (frac * circumference).max(0.0);
        let pct = (*count * 100).checked_div(total).unwrap_or(0);
        slices_svg.push_str(&format!(
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="transparent" stroke="{color}" stroke-width="26" stroke-dasharray="{dash:.2} {rest:.2}" stroke-dashoffset="{neg_off:.2}" transform="rotate(-90 {cx} {cy})"><title>{label}: {count} ({pct}%)</title></circle>"#,
            cx = cx,
            cy = cy,
            r = radius,
            color = html_escape(color),
            dash = dash,
            rest = (circumference - dash).max(0.0),
            neg_off = -offset,
            label = html_escape(label),
            count = count,
            pct = pct,
        ));
        offset += dash;

        let ly = 22 + idx * 20;
        let short_label = truncate_label(label, 24);
        legend_svg.push_str(&format!(
            r##"<g>
                <rect x="200" y="{rect_y}" width="11" height="11" rx="2" fill="{color}" />
                <text x="217" y="{txt_y}" font-size="11" fill="#374151">{label}: <tspan font-weight="600">{count}</tspan> ({pct}%)</text>
            </g>"##,
            rect_y = ly,
            txt_y = ly + 9,
            color = html_escape(color),
            label = html_escape(&short_label),
            count = count,
            pct = pct,
        ));
    }

    let height = 200usize.max(36 + non_zero.len() * 20);
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 460 {height}" class="w-full h-auto" role="img" aria-label="{title}">
            <circle cx="{cx}" cy="{cy}" r="{r}" fill="transparent" stroke="#e5e7eb" stroke-width="26" />
            {slices_svg}
            <text x="{cx}" y="{tot_y}" text-anchor="middle" font-size="20" font-weight="700" fill="#111827">{total}</text>
            <text x="{cx}" y="{sub_y}" text-anchor="middle" font-size="10" fill="#6b7280">Total Issues</text>
            {legend_svg}
        </svg>"##,
        height = height,
        title = html_escape(title),
        cx = cx,
        cy = cy,
        r = radius,
        slices_svg = slices_svg,
        tot_y = cy + 3.0,
        sub_y = cy + 18.0,
        total = total,
        legend_svg = legend_svg,
    )
}

/// Renders a multi-series SVG issues trend chart (Opened, Resolved, Cumulative Open).
pub fn render_issues_trend_svg(title: &str, points: &[TrendPoint]) -> String {
    if points.is_empty() {
        return format!(
            r#"<div class="text-xs text-gray-500 text-center py-8" role="region" aria-label="{}">No trend data available.</div>"#,
            html_escape(title)
        );
    }

    let max_val = points
        .iter()
        .flat_map(|p| [p.opened, p.resolved, p.cumulative_open])
        .max()
        .unwrap_or(0)
        .max(1);

    let width = 640.0f64;
    let height = 240.0f64;
    let pad_l = 42.0f64;
    let pad_r = 20.0f64;
    let pad_t = 32.0f64;
    let pad_b = 34.0f64;
    let plot_w = width - pad_l - pad_r;
    let plot_h = height - pad_t - pad_b;

    let mut grid_svg = String::new();
    let steps = max_val.min(4);
    for step in 0..=steps {
        let tick_val = if max_val <= 4 {
            step
        } else {
            ((max_val as f64) * (step as f64) / (steps as f64)).round() as usize
        };
        let y = pad_t + plot_h * (1.0 - (tick_val as f64) / (max_val as f64));
        grid_svg.push_str(&format!(
            r##"<line x1="{pad_l:.1}" y1="{y:.1}" x2="{x2:.1}" y2="{y:.1}" stroke="#e5e7eb" stroke-dasharray="3 3" />
            <text x="{lx:.1}" y="{ly:.1}" text-anchor="end" font-size="10" fill="#6b7280">{tick_val}</text>"##,
            pad_l = pad_l,
            x2 = pad_l + plot_w,
            y = y,
            lx = pad_l - 6.0,
            ly = y + 3.5,
            tick_val = tick_val,
        ));
    }

    let n = points.len();
    let x_for = |idx: usize| -> f64 {
        if n <= 1 {
            pad_l + plot_w / 2.0
        } else {
            pad_l + plot_w * (idx as f64) / ((n - 1) as f64)
        }
    };
    let y_for = |val: usize| -> f64 { pad_t + plot_h * (1.0 - (val as f64) / (max_val as f64)) };

    let mut pts_opened = Vec::with_capacity(n);
    let mut pts_resolved = Vec::with_capacity(n);
    let mut pts_open_cum = Vec::with_capacity(n);
    let mut markers_svg = String::new();
    let mut x_labels_svg = String::new();

    for (idx, p) in points.iter().enumerate() {
        let x = x_for(idx);
        let yo = y_for(p.opened);
        let yr = y_for(p.resolved);
        let yc = y_for(p.cumulative_open);

        pts_opened.push(format!("{:.1},{:.1}", x, yo));
        pts_resolved.push(format!("{:.1},{:.1}", x, yr));
        pts_open_cum.push(format!("{:.1},{:.1}", x, yc));

        let short_date = p.label.get(5..).unwrap_or(p.label.as_str());
        if n <= 8 || idx == 0 || idx == n - 1 || idx % 2 == 0 {
            x_labels_svg.push_str(&format!(
                r##"<text x="{x:.1}" y="{y:.1}" text-anchor="middle" font-size="10" fill="#6b7280">{lbl}</text>"##,
                x = x,
                y = height - 10.0,
                lbl = html_escape(short_date),
            ));
        }

        markers_svg.push_str(&format!(
            r##"<circle cx="{x:.1}" cy="{yc:.1}" r="3.2" fill="#f59942"><title>{date} — Backlog Open: {cum}</title></circle>
            <circle cx="{x:.1}" cy="{yo:.1}" r="3.2" fill="#2b7dbc"><title>{date} — Opened: {opened}</title></circle>
            <circle cx="{x:.1}" cy="{yr:.1}" r="3.2" fill="#629b58"><title>{date} — Resolved: {resolved}</title></circle>"##,
            x = x,
            yc = yc,
            yo = yo,
            yr = yr,
            date = html_escape(&p.label),
            cum = p.cumulative_open,
            opened = p.opened,
            resolved = p.resolved,
        ));
    }

    let poly_cum = pts_open_cum.join(" ");
    let poly_open = pts_opened.join(" ");
    let poly_res = pts_resolved.join(" ");

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0} {height:.0}" class="w-full h-auto" role="img" aria-label="{title}">
            <g class="legend" font-size="11" fill="#374151">
                <rect x="42" y="8" width="10" height="10" rx="2" fill="#2b7dbc" />
                <text x="56" y="17">Opened</text>
                <rect x="115" y="8" width="10" height="10" rx="2" fill="#629b58" />
                <text x="129" y="17">Resolved</text>
                <rect x="195" y="8" width="10" height="10" rx="2" fill="#f59942" />
                <text x="209" y="17">Open Backlog</text>
            </g>
            {grid_svg}
            <line x1="{pad_l:.1}" y1="{axis_y:.1}" x2="{axis_x2:.1}" y2="{axis_y:.1}" stroke="#9ca3af" />
            <polyline fill="none" stroke="#f59942" stroke-width="2" stroke-dasharray="4 3" points="{poly_cum}" />
            <polyline fill="none" stroke="#2b7dbc" stroke-width="2.2" points="{poly_open}" />
            <polyline fill="none" stroke="#629b58" stroke-width="2.2" points="{poly_res}" />
            {markers_svg}
            {x_labels_svg}
        </svg>"##,
        width = width,
        height = height,
        title = html_escape(title),
        grid_svg = grid_svg,
        pad_l = pad_l,
        axis_y = pad_t + plot_h,
        axis_x2 = pad_l + plot_w,
        poly_cum = poly_cum,
        poly_open = poly_open,
        poly_res = poly_res,
        markers_svg = markers_svg,
        x_labels_svg = x_labels_svg,
    )
}

#[cfg(test)]
#[path = "charts_test.rs"]
mod tests;
