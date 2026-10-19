-- The item id exactly as Dink reported it, kept for debugging. Deliberately has no foreign key:
-- item_id references collection_log_items, so it can only hold ids we recognize, whereas Dink
-- can report items that aren't in our list (too new, or not a clog item at all).
ALTER TABLE collection_log_entries ADD COLUMN dink_item_id INTEGER;
