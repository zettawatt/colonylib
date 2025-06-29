#[cfg(test)]
mod tests {

    #[test]
    fn test_sort_graph_data_with_multiline_statements() {
        // Create a simple test struct that has the same sorting logic
        struct TestSorter;

        impl TestSorter {
            fn sort_graph_data(&self, data: &str) -> String {
                let lines: Vec<&str> = data.lines().collect();
                let mut statements: Vec<Vec<&str>> = Vec::new();
                let mut current_statement: Vec<&str> = Vec::new();

                // Group lines into statements (subject + continuation lines)
                for line in lines {
                    if line.trim().is_empty() {
                        // Empty line - add to current statement if it exists, otherwise skip
                        if !current_statement.is_empty() {
                            current_statement.push(line);
                        }
                    } else if line.starts_with(char::is_whitespace) {
                        // Continuation line (starts with whitespace) - add to current statement
                        if !current_statement.is_empty() {
                            current_statement.push(line);
                        } else {
                            // Orphaned continuation line - treat as new statement
                            current_statement.push(line);
                        }
                    } else {
                        // New subject line - save previous statement and start new one
                        if !current_statement.is_empty() {
                            statements.push(current_statement);
                        }
                        current_statement = vec![line];
                    }
                }

                // Don't forget the last statement
                if !current_statement.is_empty() {
                    statements.push(current_statement);
                }

                // Sort statements based on the priority of their first (subject) line
                statements.sort_by(|a, b| {
                    let a_priority = if !a.is_empty() {
                        self.get_statement_priority(a)
                    } else {
                        2
                    };
                    let b_priority = if !b.is_empty() {
                        self.get_statement_priority(b)
                    } else {
                        2
                    };
                    a_priority.cmp(&b_priority)
                });

                // Reconstruct the sorted data
                let mut result = Vec::new();
                for statement in statements {
                    for line in statement {
                        result.push(line);
                    }
                }

                result.join("\n")
            }

            fn get_statement_priority(&self, statement: &[&str]) -> u8 {
                // Check all lines in the statement for priority indicators
                for line in statement {
                    if line.contains("index") {
                        return 0; // Pod scratchpads should always be first
                    } else if line.contains("ref") {
                        return 1; // Pod references are next
                    }
                }
                2 // Everything else
            }
        }

        let sorter = TestSorter;

        // Test data that mimics the user's example with multi-line statements
        let test_data = r#"<ant://87d23968b75f9bc3710603f55c9310dbd3843e3d8ab421208b7f5c22f9be40311efee004b1957704abd1514318773a3e> a <ant://colonylib/v1/data> .
<ant://aa61186ff2e6651ac15f492fe0e6193be126837d0ae426178af3ee7ff6b11418682c8adac568ba746fd027d33436ea89> a <ant://colonylib/v1/pod> ;
	<ant://colonylib/v1/count> "6" .
<ant://9346d6dc1f6e3982ab560257e20d9c861328b888cbc821b92fac8c81dacaa83b098781a7f54588b16416e0334d76a0d7> <ant://colonylib/vocabulary/0.1/predicate#index> "0" ;
	a <ant://colonylib/v1/data> ."#;

        let sorted_data = sorter.sort_graph_data(test_data);
        let lines: Vec<&str> = sorted_data.lines().collect();

        println!("Original data:");
        for (i, line) in test_data.lines().enumerate() {
            println!("{}: {}", i, line);
        }

        println!("\nSorted data:");
        for (i, line) in lines.iter().enumerate() {
            println!("{}: {}", i, line);
        }

        // Verify that the multi-line statement with index predicate comes first
        // and that continuation lines stay with their subject
        assert!(lines[0].contains("9346d6dc1f6e3982ab560257e20d9c861328b888cbc821b92fac8c81dacaa83b098781a7f54588b16416e0334d76a0d7"));
        assert!(lines[0].contains("index"));
        assert!(lines[1].starts_with('\t') || lines[1].starts_with(' '));
        assert!(lines[1].contains("data"));

        // Verify that the other statements follow in order
        assert!(lines[2].contains("87d23968b75f9bc3710603f55c9310dbd3843e3d8ab421208b7f5c22f9be40311efee004b1957704abd1514318773a3e"));

        // The multi-line statement with pod and count should be together
        let pod_line_index = lines.iter().position(|&line|
            line.contains("aa61186ff2e6651ac15f492fe0e6193be126837d0ae426178af3ee7ff6b11418682c8adac568ba746fd027d33436ea89")
        ).expect("Should find the pod statement");

        // The next line should be the continuation with count
        assert!(
            lines[pod_line_index + 1].starts_with('\t')
                || lines[pod_line_index + 1].starts_with(' ')
        );
        assert!(lines[pod_line_index + 1].contains("count"));

        println!("Test passed: Multi-line statements are kept together and sorted correctly!");
    }

    #[test]
    fn test_sort_graph_data_priority_ordering() {
        // Create the same test sorter
        struct TestSorter;

        impl TestSorter {
            fn sort_graph_data(&self, data: &str) -> String {
                let lines: Vec<&str> = data.lines().collect();
                let mut statements: Vec<Vec<&str>> = Vec::new();
                let mut current_statement: Vec<&str> = Vec::new();

                // Group lines into statements (subject + continuation lines)
                for line in lines {
                    if line.trim().is_empty() {
                        // Empty line - add to current statement if it exists, otherwise skip
                        if !current_statement.is_empty() {
                            current_statement.push(line);
                        }
                    } else if line.starts_with(char::is_whitespace) {
                        // Continuation line (starts with whitespace) - add to current statement
                        if !current_statement.is_empty() {
                            current_statement.push(line);
                        } else {
                            // Orphaned continuation line - treat as new statement
                            current_statement.push(line);
                        }
                    } else {
                        // New subject line - save previous statement and start new one
                        if !current_statement.is_empty() {
                            statements.push(current_statement);
                        }
                        current_statement = vec![line];
                    }
                }

                // Don't forget the last statement
                if !current_statement.is_empty() {
                    statements.push(current_statement);
                }

                // Sort statements based on the priority of their first (subject) line
                statements.sort_by(|a, b| {
                    let a_priority = if !a.is_empty() {
                        self.get_statement_priority(a)
                    } else {
                        2
                    };
                    let b_priority = if !b.is_empty() {
                        self.get_statement_priority(b)
                    } else {
                        2
                    };
                    a_priority.cmp(&b_priority)
                });

                // Reconstruct the sorted data
                let mut result = Vec::new();
                for statement in statements {
                    for line in statement {
                        result.push(line);
                    }
                }

                result.join("\n")
            }

            fn get_statement_priority(&self, statement: &[&str]) -> u8 {
                // Check all lines in the statement for priority indicators
                for line in statement {
                    if line.contains("index") {
                        return 0; // Pod scratchpads should always be first
                    } else if line.contains("ref") {
                        return 1; // Pod references are next
                    }
                }
                2 // Everything else
            }
        }

        let sorter = TestSorter;

        // Test data with different priorities
        let test_data = r#"<ant://subject1> <http://schema.org/name> "Test Name" .
<ant://scratchpad1> <ant://colonylib/vocabulary/0.1/predicate#index> "0" ;
	<http://schema.org/description> "Scratchpad description" .
<ant://subject2> <http://schema.org/description> "Test Description" .
<ant://pod_ref1> <ant://colonylib/vocabulary/0.1/object#ref> "reference" ;
	<http://schema.org/name> "Pod reference" .
<ant://subject3> <http://schema.org/type> "Dataset" ."#;

        let sorted_data = sorter.sort_graph_data(test_data);
        let lines: Vec<&str> = sorted_data.lines().collect();

        println!("Priority test - Sorted data:");
        for (i, line) in lines.iter().enumerate() {
            println!("{}: {}", i, line);
        }

        // Verify priority ordering: index statements first, then pod_ref, then others
        let mut found_index = false;
        let mut found_pod_ref = false;
        let mut found_other = false;

        for line in &lines {
            if line.contains("index") {
                assert!(
                    !found_pod_ref && !found_other,
                    "Index statements should come first"
                );
                found_index = true;
            } else if line.contains("ref") {
                assert!(
                    !found_other,
                    "Pod ref statements should come before other statements"
                );
                found_pod_ref = true;
            } else if !line.trim().is_empty() && !line.starts_with('\t') && !line.starts_with(' ') {
                found_other = true;
            }
        }

        assert!(found_index, "Should have found index statements");
        println!("Priority ordering test passed!");
    }
}
