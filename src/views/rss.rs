use crate::views::{format_rfc2822_timestamp, xml_escape};

#[derive(Debug, Clone)]
pub struct IssueRssEntry {
    pub id: i64,
    pub summary: String,
    pub description: String,
    pub category: String,
    pub project_name: String,
    pub reporter: String,
    pub status_label: String,
    pub priority_label: String,
    pub severity_label: String,
    pub link: String,
    pub pub_date_ts: i64,
}

#[derive(Debug, Clone)]
pub struct NewsRssEntry {
    pub id: i64,
    pub headline: String,
    pub body: String,
    pub project_name: String,
    pub author: String,
    pub link: String,
    pub pub_date_ts: i64,
    pub announcement: i32,
}

struct RssChannelItem {
    title: String,
    link: String,
    guid: String,
    description: String,
    creator: String,
    category: Option<String>,
    pub_date_ts: i64,
}

fn render_rss_document(
    title: &str,
    channel_link: &str,
    description: &str,
    self_link: &str,
    items: &[RssChannelItem],
) -> String {
    let last_build_ts = items
        .iter()
        .map(|item| item.pub_date_ts)
        .filter(|&ts| ts > 0)
        .max()
        .unwrap_or(0);

    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <rss version=\"2.0\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:atom=\"http://www.w3.org/2005/Atom\">\n\
         <channel>\n",
    );
    out.push_str(&format!(
        "  <title>{}</title>\n\
         \x20 <link>{}</link>\n\
         \x20 <description>{}</description>\n\
         \x20 <language>en-us</language>\n\
         \x20 <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\" />\n\
         \x20 <lastBuildDate>{}</lastBuildDate>\n",
        xml_escape(title),
        xml_escape(channel_link),
        xml_escape(description),
        xml_escape(self_link),
        xml_escape(&format_rfc2822_timestamp(last_build_ts)),
    ));

    for item in items {
        let cat_xml = item
            .category
            .as_deref()
            .filter(|c| !c.trim().is_empty())
            .map(|c| format!("\n    <category>{}</category>", xml_escape(c)))
            .unwrap_or_default();
        out.push_str(&format!(
            "  <item>\n\
             \x20   <title>{}</title>\n\
             \x20   <link>{}</link>\n\
             \x20   <guid isPermaLink=\"true\">{}</guid>\n\
             \x20   <pubDate>{}</pubDate>\n\
             \x20   <dc:creator>{}</dc:creator>{}\n\
             \x20   <description>{}</description>\n\
             \x20 </item>\n",
            xml_escape(&item.title),
            xml_escape(&item.link),
            xml_escape(&item.guid),
            xml_escape(&format_rfc2822_timestamp(item.pub_date_ts)),
            xml_escape(&item.creator),
            cat_xml,
            xml_escape(&item.description),
        ));
    }

    out.push_str("</channel>\n</rss>\n");
    out
}

pub fn render_issues_rss(
    channel_title: &str,
    channel_link: &str,
    self_link: &str,
    entries: &[IssueRssEntry],
) -> String {
    let items: Vec<RssChannelItem> = entries
        .iter()
        .map(|entry| {
            let category = if entry.project_name.is_empty() {
                entry.category.clone()
            } else if entry.category.is_empty() {
                entry.project_name.clone()
            } else {
                format!("[{}] {}", entry.project_name, entry.category)
            };

            let mut desc_parts = vec![
                format!("Project: {}", entry.project_name),
                format!("Status: {}", entry.status_label),
                format!("Priority: {}", entry.priority_label),
                format!("Severity: {}", entry.severity_label),
            ];
            if !entry.description.trim().is_empty() {
                desc_parts.push(String::new());
                desc_parts.push(entry.description.trim().to_string());
            }

            RssChannelItem {
                title: format!("{:07}: {}", entry.id, entry.summary),
                link: entry.link.clone(),
                guid: entry.link.clone(),
                description: desc_parts.join("\n"),
                creator: entry.reporter.clone(),
                category: Some(category),
                pub_date_ts: entry.pub_date_ts,
            }
        })
        .collect();

    render_rss_document(
        channel_title,
        channel_link,
        "MantisBT Issues RSS Feed",
        self_link,
        &items,
    )
}

pub fn render_news_rss(
    channel_title: &str,
    channel_link: &str,
    self_link: &str,
    entries: &[NewsRssEntry],
) -> String {
    let items: Vec<RssChannelItem> = entries
        .iter()
        .map(|entry| {
            let title = if entry.announcement != 0 {
                format!("[Announcement] {}", entry.headline)
            } else {
                entry.headline.clone()
            };
            RssChannelItem {
                title,
                link: entry.link.clone(),
                guid: format!("{}#news-{}", entry.link, entry.id),
                description: entry.body.clone(),
                creator: entry.author.clone(),
                category: (!entry.project_name.is_empty()).then(|| entry.project_name.clone()),
                pub_date_ts: entry.pub_date_ts,
            }
        })
        .collect();

    render_rss_document(
        channel_title,
        channel_link,
        "MantisBT News RSS Feed",
        self_link,
        &items,
    )
}

#[cfg(test)]
#[path = "rss_test.rs"]
mod tests;
