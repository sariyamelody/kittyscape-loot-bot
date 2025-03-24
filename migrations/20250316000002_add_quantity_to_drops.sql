-- Add migration script here

-- Add quantity column to drops table
ALTER TABLE drops ADD COLUMN quantity INTEGER NOT NULL DEFAULT 1;
