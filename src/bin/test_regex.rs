use regex::Regex;

fn main() {
    // Define the regex pattern we want to test
    let pattern = r"Just got (?:(\d+)x\s+)?\[(.+?)(?:\]\(.+?\)|\])\s+from(?:\s+lvl\s+\d+)?\s+\[(.+?)(?:\]\(.+?\)|\])";
    let re = Regex::new(pattern).unwrap();
    
    // Test cases
    let test_cases = vec![
        "Just got [Coal] from [Monster]",
        "Just got 5x [Coal] from [Monster]",
        "Just got [Coal](https://oldschool.runescape.wiki/w/Special:Search?search=Coal) from [Monster](https://oldschool.runescape.wiki/w/Special:Search?search=Monster)",
        "Just got 5x [Coal](https://oldschool.runescape.wiki/w/Special:Search?search=Coal) from lvl 98 [Sulphur Nagua](https://oldschool.runescape.wiki/w/Special:Search?search=Sulphur%20Nagua)",
    ];
    
    // Test each case
    for (i, test_case) in test_cases.iter().enumerate() {
        println!("Test Case #{}: {}", i + 1, test_case);
        
        if let Some(captures) = re.captures(test_case) {
            println!("  ✅ Matched!");
            
            // Extract quantity (or default to 1)
            let quantity = captures.get(1).map_or("1", |m| m.as_str());
            println!("  Quantity: {}", quantity);
            
            // Extract item name
            let item_name = captures.get(2).map_or("", |m| m.as_str());
            println!("  Item: {}", item_name);
            
            // Extract source/monster
            let source = captures.get(3).map_or("", |m| m.as_str());
            println!("  Source: {}", source);
        } else {
            println!("  ❌ No match!");
        }
        
        println!("");
    }
} 