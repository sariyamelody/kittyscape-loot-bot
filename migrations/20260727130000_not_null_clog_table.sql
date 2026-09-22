DROP VIEW IF EXISTS v_categories_clogs;
DROP VIEW IF EXISTS v_item_data;
DROP VIEW IF EXISTS v_users;

-- SQLite ties foreign key enforcement to the underlying table object, not just its name, so
-- dropping and recreating collection_log_items below fails at commit as soon as
-- collection_log_entries has any rows referencing it - PRAGMA foreign_keys=0 is a no-op inside
-- a transaction (which is how sqlx always runs migrations), and PRAGMA defer_foreign_keys
-- doesn't cover a dropped-and-recreated parent table either. Dropping collection_log_entries'
-- reference first (so nothing points at collection_log_items while it's swapped), then
-- restoring it afterward, keeps foreign key enforcement satisfied throughout instead of
-- depending on a pragma sqlx can't actually apply.
CREATE TABLE new_collection_log_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    discord_id TEXT NOT NULL,
    item_name TEXT NOT NULL,
    points INTEGER NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP, item_id INTEGER,
    FOREIGN KEY(discord_id) REFERENCES users(discord_id)
);
INSERT INTO new_collection_log_entries SELECT * FROM collection_log_entries;
DROP TABLE collection_log_entries;
ALTER TABLE new_collection_log_entries RENAME TO collection_log_entries;

--We could probably just drop the table altogether but this is "safest"
CREATE TABLE "new_collection_log_items" (
	"item_id"	INTEGER NOT NULL,
	"item_name"	TEXT NOT NULL,
	"preferred_name"	TEXT NOT NULL,
	"percentage"	TEXT NOT NULL,
	"categories"	TEXT NOT NULL,
	"whitelist"	INTEGER NOT NULL DEFAULT 0,
	UNIQUE("item_id")
);
INSERT INTO new_collection_log_items SELECT * FROM collection_log_items;
DROP TABLE collection_log_items;
ALTER TABLE new_collection_log_items RENAME TO collection_log_items;

CREATE TABLE newer_collection_log_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    discord_id TEXT NOT NULL,
    item_name TEXT NOT NULL,
    points INTEGER NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP, item_id INTEGER REFERENCES "collection_log_items"("item_id"),
    FOREIGN KEY(discord_id) REFERENCES users(discord_id)
);
INSERT INTO newer_collection_log_entries SELECT * FROM collection_log_entries;
DROP TABLE collection_log_entries;
ALTER TABLE newer_collection_log_entries RENAME TO collection_log_entries;

PRAGMA foreign_key_check;

--Create views (from earlier migration, needs to be redone with the table change)
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