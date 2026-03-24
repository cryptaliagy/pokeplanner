use colored::Colorize;

use crate::unusable::UnusableStore;
use crate::UnusableAction;

pub async fn handle_unusable_action(
    action: UnusableAction,
    store: &mut UnusableStore,
) -> anyhow::Result<()> {
    match action {
        UnusableAction::Add { names } => {
            if names.is_empty() {
                anyhow::bail!("Provide at least one pokemon form name");
            }
            let added = store.add(&names).await?;
            if added.is_empty() {
                println!("All specified pokemon were already marked unusable.");
            } else {
                for name in &added {
                    println!("  {} {}", "+".green(), name);
                }
                println!("Marked {} pokemon as unusable.", added.len());
            }
        }
        UnusableAction::Remove { names } => {
            if names.is_empty() {
                anyhow::bail!("Provide at least one pokemon form name");
            }
            let removed = store.remove(&names).await?;
            if removed.is_empty() {
                println!("None of the specified pokemon were in the unusable list.");
            } else {
                for name in &removed {
                    println!("  {} {}", "-".red(), name);
                }
                println!("Removed {} pokemon from unusable list.", removed.len());
            }
        }
        UnusableAction::List => {
            let list = store.list();
            if list.is_empty() {
                println!("No pokemon marked as unusable.");
            } else {
                println!(
                    "{}",
                    format!("{} pokemon marked as unusable:", list.len()).bold()
                );
                for name in &list {
                    println!("  {}", name);
                }
            }
        }
        UnusableAction::Clear => {
            let count = store.clear().await?;
            if count > 0 {
                println!("Cleared {} pokemon from unusable list.", count);
            } else {
                println!("Unusable list was already empty.");
            }
        }
    }
    Ok(())
}
