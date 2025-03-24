-- Add default rank tiers
INSERT INTO rank_thresholds (points, role_name) VALUES
    (0, 'Small Fry'),
    (1000, 'Purrveyor'),
    (3000, 'Journeycat'),
    (8000, 'Meowster'),
    (15000, 'Pawfficer'),
    (30000, 'Mewtenant'),
    (50000, 'Admeowral'),
    (75000, 'Grandmeowster'),
    -- Prestige ranks
    (100000, 'Prestige Grandmeowster I'),
    (110000, 'Prestige Grandmeowster II'),
    (120000, 'Prestige Grandmeowster III'),
    (130000, 'Prestige Grandmeowster IV'),
    (140000, 'Prestige Grandmeowster V'),
    -- Exalted ranks
    (150000, 'Exalted Grandmeowster I'),
    (160000, 'Exalted Grandmeowster II'),
    (170000, 'Exalted Grandmeowster III'),
    (180000, 'Exalted Grandmeowster IV'),
    (190000, 'Exalted Grandmeowster V'),
    -- Divine ranks
    (200000, 'Divine Grandmeowster I'),
    (210000, 'Divine Grandmeowster II'),
    (220000, 'Divine Grandmeowster III'),
    (230000, 'Divine Grandmeowster IV'),
    (240000, 'Divine Grandmeowster V'),
    -- Final rank
    (250000, 'Eternal Grandmeowster'); 