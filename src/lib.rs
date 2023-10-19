use std::{hash::Hash, collections::{HashMap, HashSet}};

use crate::collection;

trait TableRecord: Clone {
	type Key: Hash + Eq + Clone;
	type Category: Hash + Eq + Clone;
	fn categories(&self) -> Vec<Self::Category>;
	fn key(&self) -> Self::Key;
}

struct Table<T: TableRecord> { // TODO: clone because closure in .upsert()
	data: HashMap<T::Key, T>,
	index: HashMap<T::Category, Vec<T::Key>>
}

#[derive(Debug)]
pub struct KeyBusy;
impl std::error::Error for KeyBusy {}
impl std::fmt::Display for KeyBusy {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("key is busy"))
    }
}

impl<T: TableRecord> Table<T> {
	fn new() -> Self {
		Self { data: collection!(), index: collection!() }
	}

	fn len(&self) -> usize {
		self.data.len()
	}

	fn contains_key(&self, key: &T::Key) -> bool {
		self.data.contains_key(key)
	}

	fn contains_val(&self, val: &T) -> bool {
		self.data.contains_key(&val.key())
	}

	fn contains_cat(&self, cat: &T::Category) -> bool {
		self.index.contains_key(cat)
	}

	fn insert(&mut self, val: T) -> Result<(), KeyBusy> {
		let key = val.key();
		if self.data.contains_key(&key) {
			return Err(KeyBusy);
		}
		for cat in val.categories() {
			self.index.entry(cat).or_insert_with(|| vec![]).push(key.clone());
		}
		self.data.insert(key, val);
		Ok(())
	}

	fn upsert(&mut self, key: T::Key, val: T) {
		let new_cats: HashSet<T::Category> = val.categories().into_iter().collect();
		// do update
		if self.contains_key(&key) {
			self.update(key, &|old_val| *old_val = val.clone());
		} else {
			self.insert(val).unwrap();
		}
	}

	fn update(&mut self, key: T::Key, cb: &impl Fn(&mut T)) -> bool {
		let Some(mut val) = self.data.get_mut(&key) else { return false; };
		let old_cats: HashSet<T::Category> = val.categories().into_iter().collect();
		cb(&mut val);
		let new_cats = val.categories().into_iter().collect();

		for c in old_cats.difference(&new_cats) {
			self.index.entry(c.clone()).and_modify(|v| v.retain(|k| *k != key));
		}
		for c in new_cats.difference(&old_cats) {
			self.index.entry(c.clone()).or_insert_with(|| vec![]).push(key.clone());
		}
		true
	}

	fn update_by_cat(&mut self, cat: T::Category, cb: impl Fn(&mut T)) -> usize {
		// update multiple records found by category
		let Some(keys) = self.index.get(&cat) else { return 0; };
		let keys: Vec<T::Key> = keys.into_iter().map(|k| k.clone()).collect(); // TODO FIXME: ugly but required, because self.index.get borrows self immutably and it's still borrowed, while self.update requires mutable borrow.
		let updated = keys.len();
		for k in keys {
			self.update(k.clone(), &cb);
		}
		updated
	}

	fn remove(&mut self, key: &T::Key) -> Option<T> {
		// get categories
		let value = self.data.remove(key)?;
		for cat in value.categories() {
			self.index.remove(&cat);
		}
		Some(value)
	}

	fn get(&self, key: &T::Key) -> Option<&T> {
		self.data.get(key)
	}

	fn find(&self, cat: &T::Category) -> Vec<&T> { // TODO: replace with iterator struct
		self.index.get(cat).unwrap_or(&vec![]).iter().filter_map(|k| self.data.get(k)).collect()
	}

	fn find_many(&self, cats: &[T::Category]) -> Vec<&T> { // TODO: replace with iterator struct
		cats.iter()
			.filter_map(|c| self.index.get(c))
			.flatten() // Vec<T>s into T-s
			.filter_map(|k| self.data.get(k)) //
			.collect()
	}

	fn iter(&self) -> impl Iterator<Item = (&T::Key, &T)> {
		self.data.iter()
	}

	fn values(&self) -> impl Iterator<Item = &T> {
		self.data.values()
	}

	fn iter_keys(&self) -> impl Iterator<Item = &T::Key> {
		self.data.keys()
	}

	fn iter_cats(&self) -> impl Iterator<Item = &T::Category> {
		self.index.keys()
	}
}


#[cfg(test)]
pub mod multimap_tests {
	use super::*;

	#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
	struct ScienceId(usize);
	#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
	struct AuthorId(usize);
	#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
	struct BookId(usize);

	#[derive(Debug, Clone, Hash, PartialEq, Eq)]
	enum BookCategory {
		Science(ScienceId),
		Author(AuthorId),
	}


	#[derive(Debug, Clone, PartialEq, Eq, Hash)]  // PartialEq, Eq & Hash are for sets comparisons in test
	struct Book {
		id: BookId,
		title: String,
		science: ScienceId,
		author: AuthorId,
	}

	impl TableRecord for Book {
		type Key = BookId;
		type Category = BookCategory;
		fn categories(&self) -> Vec<Self::Category> {
			vec![BookCategory::Science(self.science.clone()), BookCategory::Author(self.author.clone())]
		}
		fn key(&self) -> Self::Key {
			self.id.clone()
		}
	}

	#[test]
	fn test_index_table() {
		let mut it: Table<Book> = Table::new();
		let s2 = ScienceId(2);
		let s3 = ScienceId(3);
		let s4 = ScienceId(4);
		let s5 = ScienceId(5);

		let a0 = AuthorId(10);
		let a1 = AuthorId(11);
		let a2 = AuthorId(12);

		let books = vec![
			Book { id: BookId(1), title: "Book №1".into(), science: s2, author: a0 },
			Book { id: BookId(2), title: "Book №2".into(), science: s2, author: a1 },
			Book { id: BookId(3), title: "Book №3".into(), science: s2, author: a2 },

			Book { id: BookId(4), title: "Book №4".into(), science: s3, author: a0 },
			Book { id: BookId(5), title: "Book №5".into(), science: s3, author: a1 },
			Book { id: BookId(6), title: "Book №6".into(), science: s3, author: a2 },

			Book { id: BookId(7), title: "Book №7".into(), science: s4, author: AuthorId(13) }, // alone in both categories
		];

		for b  in books.clone().into_iter() {
			it.insert(b).unwrap();
		}

		for b in books.iter() {
			assert!(it.contains_key(&b.key()));
		}

		assert!(!it.contains_key(&BookId(1000)));
		assert_eq!(it.len(), 7);
		for science in &[s2, s3, s4] {
			assert!(it.contains_cat(&BookCategory::Science(science.clone())));
		}
		assert!(!it.contains_cat(&BookCategory::Science(s5)));

		let expected_values: HashSet<Book> = books.clone().into_iter().collect();
		let real_values: HashSet<Book> = it.values().map(|b| b.clone()).collect();
		assert_eq!(expected_values, real_values);

		for b in books.iter() {
			assert!(it.contains_val(b));
		}

		// upsert a book
		// write book5 into book2
		let b2 = books[1].clone();
		// find books by book 2 author (a1)
		let prev_author_books = it.find(&BookCategory::Author(b2.author)).len();
		it.upsert(BookId(2), Book { id: BookId(5), title: "Book №5".into(), science: s3, author: a0 });
		// less 1 book by author (a1)
		let curr_author_books = it.find(&BookCategory::Author(b2.author)).len();
		assert_eq!(prev_author_books - 1, curr_author_books);
	}
}