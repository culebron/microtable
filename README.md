# Data Structure like both HashMap and a database table

This is a data structure that allows saving objects with unique ID (key) and be searched by other discrete fields (categories). It also can be serialized with Serde (feature `"serde"`).

## Usecase

Sometimes you have a collection and need to search items of it by different attributes. For example, we want to search books not just by their unique ID, but by author, or topic.

```rust
struct Book {
		id: BookId,
		title: String,
		science: ScienceId,
		author: AuthorId,
	}
...
	let my_books = vec![
		Book { id: BookId(1), title: "Book №1".into(), science: s2, author: a0 },
		Book { id: BookId(2), title: "Book №2".into(), science: s2, author: a1 },
		Book { id: BookId(3), title: "Book №3".into(), science: s2, author: a2 },
	];
```

How can we access these books by `science` field without full scan? We'd need to manually build an index, like this:

```rust
	let mut science_index: HashMap<ScienceId, Vec<&Book>> = HashMap::new();
	for b in my_books.iter() {
		science_index.entry(b.science).or_insert_with(|| vec![]).push(b);
	}
```

This would work, but only in simple case where we manage `my_books` and `science_index` in our code separately. We couldn't put them in a struct like this, because circural references are forbidden in Rust:

```rust
struct MyData {
	books: Vec<Book>,
	books_by_science: HashMap<ScienceId, Vec<&Book>>, // forbidden
}
```

We'd work around by making hashmaps:

```rust
struct MyData {
	books: HashMap<BookId, Book>,
	books_by_science: HashMap<ScienceId, BookId>,
}
```

...but this would require to write all manual updates to `books_by_science`.

This structure does this for you.

## Usage

### Example 1, Books

If you have a unique ID and a f

```rust
use microtable::{MicroTable, MicroRecord};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum BookCategory { Science(ScienceId), Author(AuthorId) }

impl MicroRecord for Book {
	type Key = BookId; // key must be unique
	type Category = BookCategory; // categories may be duplicated
	fn categories(&self) -> Vec<Self::Category> {
		vec![BookCategory::Science(self.science.clone()), BookCategory::Author(self.author.clone())]
	}
	fn key(&self) -> Self::Key { self.id.clone() }
}
...
struct MyData {
	books: MicroTable<Book>
}
...
	let my_data = MyData::new();
	for b in vec![Book { id: BookId(1), title: "Book №1".into(), science: ScienceId(2), author: AuthorId(1) },
		Book { id: BookId(2), title: "Book №2".into(), science: ScienceId(1), author: AuthorId(2) },
		Book { id: BookId(3), title: "Book №3".into(), science: ScienceId(2), author: AuthorId(3) }
	].into_iter() {
		my_data.books.insert(b).unwrap(); // may return Err if keys collide.
	}
	...
	for book in my_data.find(ScienceId(2)) {
		println!("book {book:?} science {:?}", book.science);
	}
	println!("book: {:?}", my_data.get(&BookId(1)));
```

### Example 2, Graph

Let's say we have a graph with edges. Edges have an ID and vertice.

```rust
#[derive(Clone, Hash, PartialEq, Eq)]
struct VertexId(usize);
#[derive(Clone, Hash, PartialEq, Eq)]
struct EdgeId(usize);

struct Edge {
	id: EdgeId,
	v1: VertexId,
	v2: VertexId,
	weight: usize,
}

impl MicroRecord for Edge {
	type Key = EdgeId; // key must be unique
	type Category = VertexId; // categories may be duplicated
	fn categories(&self) -> Vec<Self::Category> {
		vec![self.v1.clone(), self.v2.clone()]
	}
	fn key(&self) -> Self::Key { self.id.clone() } // there may be multiple edges between same vertice
}

struct SomeGraph {
	edges: MicroTable<Edge>
}

...
	let graph = SomeGraph::new();
	...
	let some_edge = graph.edges.get(&EdgeId(123)).unwrap();
	let neighbors_left: Vec<&Edge> = graph.edges.find(&some_edge.v1);
	let neighbors_right: Vec<&Edge> = graph.edges.find(&some_edge.v2);
	let all_neighbors: Vec<&Edge> = graph.edges.find_many([&some_edge.v1, &some_edge.v2]);
```
