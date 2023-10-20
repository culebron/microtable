use std::{hash::Hash, collections::{HashMap, HashSet}};
#[cfg(feature="serde")]
use serde::{Serialize, Deserialize};

pub trait TableRecord: Clone {
	type Key: Hash + Eq + Clone;
	type Category: Hash + Eq + Clone;
	fn categories(&self) -> Vec<Self::Category>;
	fn key(&self) -> Self::Key;
}

#[derive(Debug, Clone)]
pub struct Table<T: TableRecord> { // TODO: clone because closure in .upsert()
	data: HashMap<T::Key, T>,
	index: HashMap<T::Category, HashSet<T::Key>>
}

#[derive(Debug)]
pub enum QueryError {
	KeyCollision,
	KeyNotFound,
}
impl std::error::Error for QueryError {}
impl std::fmt::Display for QueryError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let s = match self {
			Self::KeyCollision => "key is busy",
			Self::KeyNotFound => "key not found"
		};
		f.write_fmt(format_args!("{}", s))
    }
}

impl<T: TableRecord> Table<T> {
	pub fn new() -> Self {
		Self { data: HashMap::new(), index: HashMap::new() }
	}

	pub fn clear(&mut self) {
		self.data.clear();
		self.index.clear();
	}

	pub fn len(&self) -> usize {
		self.data.len()
	}

	pub fn contains_key(&self, key: &T::Key) -> bool {
		self.data.contains_key(key)
	}

	pub fn contains_val(&self, val: &T) -> bool {
		self.data.contains_key(&val.key())
	}

	pub fn contains_cat(&self, cat: &T::Category) -> bool {
		self.index.contains_key(cat)
	}

	pub fn insert(&mut self, val: T) -> Result<(), QueryError> {
		let key = val.key();
		if self.data.contains_key(&key) {
			return Err(QueryError::KeyCollision);
		}
		for cat in val.categories() {
			self.index.entry(cat).or_insert_with(|| HashSet::new()).insert(key.clone());
		}
		self.data.insert(key, val);
		Ok(())
	}

	/// Finds the object by old key, updates it. The key in the table is not updated.
	// TODO: make it updated
	pub fn upsert(&mut self, key: T::Key, new_val: T) -> Result<(), QueryError> {
		let new_key = new_val.key();
		if new_key != key && self.data.contains_key(&new_key) {
			return Err(QueryError::KeyCollision);
		}
		if self.contains_key(&key) {
			self.update_with(key, &|old_val| *old_val = new_val.clone()).unwrap(); // unwrap because checked in .contains_key
		} else {
			self.insert(new_val).unwrap(); // unwrap because checked
		}
		Ok(())
	}

	pub fn update_with(&mut self, old_key: T::Key, cb: &impl Fn(&mut T)) -> Result<(), QueryError>  {
		let Some(val) = self.data.get(&old_key) else { return Err(QueryError::KeyNotFound); };
		let mut val = val.clone();
		let old_cats = vec2hashset(val.categories());
		cb(&mut val);
		let new_key = val.key();
		if new_key != old_key {
			self.insert(val)?;
			self.remove(&old_key);
		} else {
			let new_cats = vec2hashset(val.categories());

			for c in old_cats.difference(&new_cats) {
				self.index.entry(c.clone()).and_modify(|e| { e.remove(&old_key); });
			}
			for c in new_cats.difference(&old_cats) {
				self.index.entry(c.clone()).or_insert_with(|| HashSet::new()).insert(old_key.clone());
			}
			self.clear_empty_categories();
		}
		Ok(())
	}

	pub fn update_by_cat(&mut self, cat: T::Category, cb: impl Fn(&mut T)) -> Result<usize, QueryError> {
		// update multiple records found by category
		let Some(keys) = self.index.get(&cat) else { return Ok(0); };
		let keys: Vec<T::Key> = keys.into_iter().map(|k| k.clone()).collect(); // ugly but required, because self.index.get borrows self immutably and it's still borrowed, while self.update requires mutable borrow.
		let update_count = keys.len();
		// can fail if there's key collision. must run check beforehand
		// callbacks are run on copies, results are stored, then if all is ok, we can save the data with upsert
		let mut updates: Vec<(T::Key, T)> = vec![];
		for old_key in keys.into_iter() {
			let mut item = self.data.get(&old_key).unwrap().clone();
			cb(&mut item);
			let new_key = item.key();
			if new_key != old_key && self.contains_key(&new_key) {
				return Err(QueryError::KeyCollision);
			}
			updates.push((old_key, item));
		}
		for (old_key, new_val) in updates.into_iter() {
			self.upsert(old_key, new_val).unwrap(); // already checked
		}
		Ok(update_count)
	}

	fn clear_empty_categories(&mut self) {
		self.index.retain(|_, keys| keys.len() > 0);
	}

	pub fn remove(&mut self, key: &T::Key) -> Option<T> {
		// get categories
		let value = self.data.remove(key)?;
		for cat in value.categories() {
			self.index.entry(cat).and_modify(|c| { c.remove(key); });
			self.clear_empty_categories();
		}
		Some(value)
	}

	pub fn remove_cat(&mut self, cat: &T::Category) -> Vec<T> {
		let Some(keys) = self.index.remove(cat) else { return vec![] };
		keys.iter().filter_map(|k| self.data.remove(k)).collect()
	}

	pub fn get(&self, key: &T::Key) -> Option<&T> {
		self.data.get(key)
	}

	pub fn find(&self, cat: &T::Category) -> Vec<&T> { // TODO: replace with iterator struct
		let Some(hs) = self.index.get(cat) else { return vec![] };
		hs.iter().filter_map(|k| self.data.get(k)).collect()
	}

	pub fn find_many(&self, cats: &[T::Category]) -> Vec<&T> { // TODO: replace with iterator struct
		let keys: HashSet<&T::Key> = cats.iter()
			.filter_map(|c| self.index.get(c))
			.flatten().collect();

		// Vec<T>s into T-s
		keys.iter().filter_map(|k| self.data.get(k)).collect()
	}

	pub fn iter(&self) -> impl Iterator<Item = (&T::Key, &T)> {
		self.data.iter()
	}

	pub fn values(&self) -> impl Iterator<Item = &T> {
		self.data.values()
	}

	pub fn iter_keys(&self) -> impl Iterator<Item = &T::Key> {
		self.data.keys()
	}

	pub fn iter_cats(&self) -> impl Iterator<Item = &T::Category> {
		self.index.keys()
	}
}



fn vec2hashset<T: Hash + Eq>(data: Vec<T>) -> HashSet<T> {
	data.into_iter().collect()
}

#[cfg(feature="serde")]
impl<T: TableRecord + Serialize> Serialize for Table<T> {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
		let data: Vec<T> = self.data.clone().into_values().collect();
		data.serialize(serializer)
    }
}

#[cfg(feature="serde")]
impl<'de, T: TableRecord + Deserialize<'de>> Deserialize<'de> for Table<T> {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
		let mut t: Table<T> = Table::new();
		for item in Vec::deserialize(deserializer)?.into_iter() {
			t.insert(item).unwrap();
		}
		Ok(t)
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

	fn books_fixture() -> Vec<Book> {
		let s2 = ScienceId(22);
		let s3 = ScienceId(23);
		let s4 = ScienceId(24);

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
		let s2 = ScienceId(22);
		let s3 = ScienceId(23);
		let s4 = ScienceId(24);
		let s5 = ScienceId(25);
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
		let s3 = ScienceId(23);
		let a0 = AuthorId(10);
		let a1 = AuthorId(11);
		let a2 = AuthorId(12);

		// upsert a book (update existing one)
		// write book5 into book2
		let old_len = it.len();
		let b2 = books[1].clone();
		// find books by book 2 author (a1)
		let prev_author_books = it.find(&BookCategory::Author(b2.author)).len();
		// TODO: what if we try to change the key? Should check for collisions?
		assert!(it.upsert(BookId(2), Book { id: BookId(2), title: "Book №5".into(), science: s3, author: a0 }).is_ok());
		assert!(it.contains_key(&BookId(3)));
		assert!(!it.contains_key(&BookId(365)));
		assert!(!it.index.get(&BookCategory::Author(a2)).unwrap().contains(&BookId(365)));
		assert!(it.upsert(BookId(3), Book { id: BookId(365), title: "Book №365".into(), science: s3, author: a2 }).is_ok());
		assert!(!it.contains_key(&BookId(3)));
		assert!(it.contains_key(&BookId(365)));
		assert!(it.index.get(&BookCategory::Author(a2)).unwrap().contains(&BookId(365)));
		// upserting with key collision must fail
		assert!(it.upsert(BookId(2), Book { id: BookId(365), title: "Book №365".into(), science: s3, author: a2 }).is_err());
		// less 1 book by author (a1)
		let curr_author_books = it.find(&BookCategory::Author(b2.author)).len();
		assert_eq!(prev_author_books - 1, curr_author_books);
		assert_eq!(it.len(), old_len);


		// upsert a new book
		// adds new category
		it.upsert(BookId(10), Book { id: BookId(10), title: "Book 10".into(), science: ScienceId(30), author: a1}).unwrap();
		// update book with no new category
		it.upsert(BookId(10), Book { id: BookId(10), title: "Book №10".into(), science: ScienceId(31), author: a1}).unwrap();
		assert_eq!(it.len(), old_len + 1);
	}

	#[test]
	fn test_update_by_cat() {
		let a2 = AuthorId(2);
		let mut it = table_fixture();
		let books = books_fixture();
		let b3 = books[2].clone();

		// update category: set category 23 to 24 (also checking for collision of editing/reading categories and writing)
		it.update_by_cat(BookCategory::Science(ScienceId(23)), |b| b.science = ScienceId(24)).unwrap();

		let old_len = it.len();
		it.remove(&BookId(1));
		assert_eq!(old_len - 1, it.len());

		let c = |b: &mut Book| b.author = a2;
		// update non existent book
		assert!(it.update_with(BookId(123456), &c).is_err()); // must return err
		assert!(it.update_with(BookId(4), &c).is_ok()); // must return ok

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
		let real: HashSet<_> = it.find_many(&[BookCategory::Science(ScienceId(22)), BookCategory::Author(AuthorId(10))]).iter().map(|b| b.id.0).collect();
		let expected: HashSet<usize> = HashSet::from([1, 2, 3, 4]);
		assert_eq!(real, expected);
	}
}
