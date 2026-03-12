--clogs tables
CREATE TABLE IF NOT EXISTS "category_table" (
	"category"	TEXT UNIQUE,
	"clamp"	INTEGER DEFAULT 0
);
CREATE TABLE IF NOT EXISTS "collection_log_items" (
	"item_id"	INTEGER NOT NULL,
	"item_name"	TEXT,
	"preferred_name"	TEXT,
	"percentage"	TEXT,
	"categories"	TEXT,
	"whitelist"	INTEGER DEFAULT 0,
	UNIQUE("item_id")
);

--add columns
ALTER TABLE drops ADD COLUMN item_id INTEGER; --No reference, maybe I'll move the mappings to a table too
ALTER TABLE collection_log_entries ADD COLUMN item_id INTEGER REFERENCES "collection_log_items"("item_id");

--Create views
CREATE VIEW IF NOT EXISTS v_categories_clogs (item_id, category) AS WITH RECURSIVE split(id, value, rest) AS (
   SELECT item_id, '', categories||',' FROM collection_log_items
   UNION ALL SELECT
   id,
   substr(rest, 0, instr(rest, ',')),
   substr(rest, instr(rest, ',')+1)
   FROM split WHERE rest!=''
)
SELECT id as item_id, trim(value) as category
FROM split
WHERE category!='';
CREATE VIEW IF NOT EXISTS v_item_data AS WITH linkedcats as (
                SELECT item_id, v_categories_clogs.category FROM v_categories_clogs
            ),
	clampedcats as (
	SELECT linkedcats.item_id, group_concat(category_table.category, ", ") as clamped_category, clamp
	FROM
	category_table
	INNER JOIN linkedcats ON linkedcats.category=category_table.category
	WHERE clamp = 1
	GROUP BY item_id),
	clogtable as (
    SELECT collection_log_entries.item_name as item_name, count(item_name) as clog_count, points from collection_log_entries where points > 0 group by item_name order by points ASC
)
SELECT collection_log_items.item_id as item_id, collection_log_items.item_name as item_name, preferred_name, categories, percentage, coalesce(points,0) as highest_points, whitelist, coalesce(clog_count,0) as clog_count, coalesce(clamp,0) as clamp, coalesce(clamped_category," ") as clamped_category
FROM collection_log_items
LEFT JOIN clampedcats ON clampedcats.item_id=collection_log_items.item_id
LEFT JOIN clogtable ON clogtable.item_name=collection_log_items.item_name
ORDER BY item_id;
CREATE VIEW IF NOT EXISTS v_users as 
with droptable as (
    select discord_id, sum(value / 100000) as drop_points, count(id) as drop_count from drops group by discord_id
),
clogtable as (
    select discord_id, sum(points) as clog_points, count(item_name) as clog_count from collection_log_entries group by discord_id
)
select users.discord_id, drop_points, clog_points, COALESCE(drop_points,0) + COALESCE(clog_points,0) as total_points, drop_count, clog_count from users
left join droptable on users.discord_id = droptable.discord_id
left join clogtable on users.discord_id = clogtable.discord_id;