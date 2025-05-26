-- 20250526_create_gap_analysis_views.sql

-- 1. Entries missing BOTH content and summary, grouped by feed_url
CREATE OR REPLACE VIEW v_missing_both_per_feed AS
SELECT
    feed_url,
    COUNT(*) AS missing_both
FROM current
WHERE (content IS NULL OR content = '')
  AND (summary IS NULL OR summary = '')
GROUP BY feed_url
ORDER BY missing_both DESC;

-- 2. Entries missing ONLY content (but not summary)
CREATE OR REPLACE VIEW v_only_content_missing AS
SELECT
    feed_url,
    COUNT(*) AS only_content_missing
FROM current
WHERE (content IS NULL OR content = '')
  AND NOT (summary IS NULL OR summary = '')
GROUP BY feed_url
ORDER BY only_content_missing DESC;

-- 3. Entries missing ONLY summary (but not content)
CREATE OR REPLACE VIEW v_only_summary_missing AS
SELECT
    feed_url,
    COUNT(*) AS only_summary_missing
FROM current
WHERE (summary IS NULL OR summary = '')
  AND NOT (content IS NULL OR content = '')
GROUP BY feed_url
ORDER BY only_summary_missing DESC;

-- 4. Drill-down: Specific entries missing BOTH content and summary
CREATE OR REPLACE VIEW v_missing_both_entries AS
SELECT
    feed_url,
    title,
    link,
    published
FROM current
WHERE (content IS NULL OR content = '')
  AND (summary IS NULL OR summary = '')
ORDER BY feed_url, published DESC;

-- 5. Missing both, grouped by day and feed
CREATE OR REPLACE VIEW v_missing_both_by_day AS
SELECT
    feed_url,
    DATE_TRUNC('day', published) AS pub_day,
    COUNT(*) AS missing_both
FROM current
WHERE (content IS NULL OR content = '')
  AND (summary IS NULL OR summary = '')
GROUP BY feed_url, pub_day
ORDER BY pub_day DESC, feed_url;

-- 6. Coverage summary: total, missing content, missing summary, missing both, percent missing
CREATE OR REPLACE VIEW v_feed_coverage_summary AS
SELECT
    feed_url,
    COUNT(*) AS total,
    SUM(CASE WHEN (content IS NULL OR content = '') THEN 1 ELSE 0 END) AS content_missing,
    SUM(CASE WHEN (summary IS NULL OR summary = '') THEN 1 ELSE 0 END) AS summary_missing,
    SUM(CASE WHEN (content IS NULL OR content = '') AND (summary IS NULL OR summary = '') THEN 1 ELSE 0 END) AS both_missing,
    ROUND(
        100.0 * SUM(CASE WHEN (content IS NULL OR content = '') AND (summary IS NULL OR summary = '') THEN 1 ELSE 0 END) / COUNT(*),
        1
    ) AS percent_both_missing
FROM current
GROUP BY feed_url
ORDER BY percent_both_missing DESC;

-- 7. Most recent missing entry (missing both) per feed
CREATE OR REPLACE VIEW v_most_recent_missing_both AS
SELECT
    feed_url,
    MAX(published) AS most_recent_missing
FROM current
WHERE (content IS NULL OR content = '')
  AND (summary IS NULL OR summary = '')
GROUP BY feed_url
ORDER BY most_recent_missing DESC;

-- 8. Entries where content and summary are both missing AND title is blank
CREATE OR REPLACE VIEW v_missing_both_and_title AS
SELECT
    feed_url,
    title,
    link,
    published,
    content,
    summary
FROM current
WHERE (content IS NULL OR content = '')
  AND (summary IS NULL OR summary = '')
  AND (title IS NULL OR title = '')
ORDER BY published DESC;

-- 9. Percent missing both (rounded for easy charting)
CREATE OR REPLACE VIEW v_percent_missing_both AS
SELECT
    feed_url,
    COUNT(*) AS total,
    SUM(CASE WHEN (content IS NULL OR content = '') AND (summary IS NULL OR summary = '') THEN 1 ELSE 0 END) AS missing_both,
    ROUND(
      100.0 * SUM(CASE WHEN (content IS NULL OR content = '') AND (summary IS NULL OR summary = '') THEN 1 ELSE 0 END) / COUNT(*),
      1
    ) AS percent_missing
FROM current
GROUP BY feed_url
ORDER BY percent_missing DESC;

-- 10. Most affected titles (for triage/reporting)
CREATE OR REPLACE VIEW v_most_affected_titles AS
SELECT
    feed_url,
    title,
    link,
    published,
    content,
    summary
FROM current
WHERE (content IS NULL OR content = '')
  AND (summary IS NULL OR summary = '')
ORDER BY feed_url, published DESC;

-- Add more as needed!

-- Indexes (if you find these slow, add these for performance):
CREATE INDEX IF NOT EXISTS idx_current_content_null ON current ((content IS NULL OR content = ''));
CREATE INDEX IF NOT EXISTS idx_current_summary_null ON current ((summary IS NULL OR summary = ''));
CREATE INDEX IF NOT EXISTS idx_current_feed_url ON current(feed_url);
CREATE INDEX IF NOT EXISTS idx_current_published ON current(published);

-- End of migration file

