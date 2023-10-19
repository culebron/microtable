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

	/// Finds the object by old key, updates it. The key in the table is not updated.
	// TODO: make it updated
	fn upsert(&mut self, old_key: T::Key, new_val: T) {
		let new_cats = vec2hashset(new_val.categories());
		// do update
		if self.contains_key(&old_key) {
			self.update(old_key, &|old_val| *old_val = new_val.clone());
		} else {
			self.insert(new_val).unwrap();
		}
	}

	// TODO: make it update the key change
	fn update(&mut self, key: T::Key, cb: &impl Fn(&mut T)) -> bool {
		let Some(mut val) = self.data.get_mut(&key) else { return false; };
		let old_cats = vec2hashset(val.categories());
		cb(&mut val);
		let new_cats = vec2hashset(val.categories());

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
		let keys: HashSet<&T::Key> = cats.iter()
			.filter_map(|c| self.index.get(c))
			.flatten().collect();

		 // Vec<T>s into T-s
		keys.iter().filter_map(|k| self.data.get(k)).collect()
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

fn vec2hashset<T: Hash + Eq>(data: Vec<T>) -> HashSet<T> {
	data.into_iter().collect()
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

	fn books_fixture() -> Vec<Book> {
		let s2 = ScienceId(2);
		let s3 = ScienceId(3);
		let s4 = ScienceId(4);

		let a0 = AuthorId(10);
		let a1 = AuthorId(11);
		let a2 = AuthorId(12);

		vec![
			Book { id: BookId(1), title: "Book №1".into(), science: s2, author: a0 },
			Book { id: BookId(2), title: "Book №2".into(), science: s2, author: a1 },
			Book { id: BookId(3), title: "Book №3".into(), science: s2, author: a2 },

			Book { id: BookId(4), title: "Book №4".into(), science: s3, author: a0 },
			Book { id: BookId(5), title: "Book №5".into(), science: s3, author: a1 },
			Book { id: BookId(6), title: "Book №6".into(), science: s3, author: a2 },

			Book { id: BookId(7), title: "Book №7".into(), science: s4, author: AuthorId(13) }, // alone in both categories
		]
	}

	fn table_fixture() -> Table<Book> {
		let mut it: Table<Book> = Table::new();
		let books = books_fixture();

		for b  in books.clone().into_iter() {
			it.insert(b).unwrap();
		}

		it
	}

	#[test]
	fn test_contains_key() {
		let books = books_fixture();
		let it = table_fixture();
		for b in books.iter() {
			assert!(it.contains_key(&b.key()));
		}

		assert!(!it.contains_key(&BookId(1000)));
	}

	#[test]
	fn test_categories() {
		let s2 = ScienceId(2);
		let s3 = ScienceId(3);
		let s4 = ScienceId(4);
		let s5 = ScienceId(5);
		let books = books_fixture();
		let it = table_fixture();

		assert_eq!(it.len(), 7);
		for science in &[s2, s3, s4] {
			assert!(it.contains_cat(&BookCategory::Science(science.clone())));
		}
		assert!(!it.contains_cat(&BookCategory::Science(s5)));

		let expected_values = vec2hashset(books.clone());
		let real_values = it.values().map(|b| b.clone()).collect();
		assert_eq!(expected_values, real_values);
	}

	#[test]
	fn test_contains_val() {
		let it = table_fixture();
		let books = books_fixture();
		for b in books.iter() {
			assert!(it.contains_val(b));
		}
	}

	#[test]
	fn test_insert() {
		let mut it = table_fixture();
		let books = books_fixture();
		let res = it.insert(books[0].clone()).is_err();
		assert!(res);

		println!("{}", res);
		println!("{:?}", res);
	}

	#[test]
	fn test_upsert() {
		let mut it = table_fixture();
		let books = books_fixture();
		let s3 = ScienceId(3);
		let a0 = AuthorId(10);
		let a1 = AuthorId(11);

		// upsert a book (update existing one)
		// write book5 into book2
		let old_len = it.len();
		let b2 = books[1].clone();
		// find books by book 2 author (a1)
		let prev_author_books = it.find(&BookCategory::Author(b2.author)).len();
		// TODO: what if we try to change the key? Should check for collisions?
		it.upsert(BookId(2), Book { id: BookId(2), title: "Book №5".into(), science: s3, author: a0 });
		// less 1 book by author (a1)
		let curr_author_books = it.find(&BookCategory::Author(b2.author)).len();
		assert_eq!(prev_author_books - 1, curr_author_books);
		assert_eq!(it.len(), old_len);


		// upsert a new book
		// adds new category
		it.upsert(BookId(10), Book { id: BookId(10), title: "Book 10".into(), science: ScienceId(100), author: a1});
		// update book with no new category
		it.upsert(BookId(10), Book { id: BookId(10), title: "Book №10".into(), science: ScienceId(101), author: a1});
		assert_eq!(it.len(), old_len + 1);
	}

	#[test]
	fn test_update_by_cat() {
		let a2 = AuthorId(2);
		let mut it = table_fixture();
		let books = books_fixture();
		let b3 = books[2].clone();

		// update category: set category 2 to 3 (also checking for collision of editing/reading categories and writing)
		it.update_by_cat(BookCategory::Science(ScienceId(3)), |b| b.science = ScienceId(4));

		let old_len = it.len();
		it.remove(&BookId(1));
		assert_eq!(old_len - 1, it.len());

		// update non existent book
		assert!(!it.update(BookId(123456), &|b| b.author = a2)); // must return false

		assert_eq!(it.get(&BookId(3)), Some(&b3));
		assert_eq!(it.get(&BookId(654321)), None);

	}

	#[test]
	fn test_iter() {
		let it = table_fixture();
		let real: HashSet<_> = it.iter().collect();
		let bf = books_fixture();
		let expected: HashSet<_> = bf.iter().map(|b| (&b.id, b)).collect();
		assert_eq!(expected, real);

		let real: HashSet<_> = it.values().collect();
		let expected: HashSet<_> = bf.iter().collect();
		assert_eq!(real, expected);

		let real: HashSet<_> = it.iter_keys().collect();
		let expected: HashSet<_> = bf.iter().map(|b| &b.id).collect();
		assert_eq!(real, expected);

		let real: HashSet<_> = it.iter_cats().map(|c| c.clone()).collect();
		let expected: HashSet<_> = bf.iter().map(|b| vec![BookCategory::Science(b.science), BookCategory::Author(b.author)]).flatten().collect();
		assert_eq!(real, expected);
	}

	#[test]
	fn find_many() {
		let it = table_fixture();
		let real: HashSet<_> = it.find_many(&[BookCategory::Science(ScienceId(2)), BookCategory::Author(AuthorId(10))]).iter().map(|b| b.id.0).collect();
		let expected: HashSet<usize> = collection!(1, 2, 3, 4);
		assert_eq!(real, expected);
	}

}
