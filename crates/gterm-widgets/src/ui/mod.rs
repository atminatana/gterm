//! Описание экрана (ADR-0026): временное дерево из контейнеров и контролов по `&mut` на
//! состояния приложения. Приложение строит его функцией `ui()` на каждое событие и на каждый
//! кадр; одно описание служит раскладке, отрисовке, обходу фокуса и попаданию мыши — порядок
//! везде один, по построению. Флаги фокуса, области и захвата живут в состояниях контролов
//! ([`NodeState`](crate::control::NodeState)), поэтому идентификаторов нет и исчезновение
//! контрола из описания обнаруживается.
//!
//! Порядок в обработчике приложения: `let r = self.ui().key(event, ctx);` — описание сброшено
//! в конце оператора — разбор `r` с изменением `self` — `self.ui().render(area, ctx.screen)`.

mod element;
mod focus;
mod modal;
mod route;

pub use modal::Modal;

use gterm::buffer::ScreenBuffer;
use gterm::hotkey::HotkeyTracker;

use crate::buffer::BufferExt;
use crate::clipboard::Clipboard;
use crate::control::{Control, Drawable, WidgetEvents};
use crate::geometry::Rect;
use crate::hotkeys::HotkeyMap;
use crate::layout::Layout;
use crate::style::Style;
use crate::theme::Theme;
use crate::widgets::{Block, StatefulWidget, Widget};

use element::{Custom, Element, Owned, Stateful, Stateless};

/// Описание экрана с сообщениями приложения типа `M`.
pub struct Ui<'a, M> {
	/// Узлы в порядке обхода в глубину: родитель раньше детей, корень — `0`.
	nodes: Vec<Node<'a, M>>,
	/// Области узлов после [`Ui::layout`], параллельно `nodes`.
	areas: Vec<Rect>,
	/// Модальный слой поверх описания: пока он есть, весь ввод идёт ему.
	modal: Option<Box<Modal<'a, M>>>,
	/// Карта сочетаний экрана и трекер приложения (ADR-0029).
	hotkeys: Option<Hotkeys<'a, M>>,
	/// Буфер обмена приложения для команд правки (ADR-0030).
	clipboard: Option<&'a mut dyn Clipboard>,
	/// Тема оформления (ADR-0037): доходит до каждого узла при отрисовке.
	theme: Option<&'a Theme>,
}

/// Трекер комбинации из состояния приложения и карта, действующая без модального слоя.
struct Hotkeys<'a, M> {
	tracker: &'a mut HotkeyTracker,
	map: &'a HotkeyMap<M>,
}

struct Node<'a, M> {
	parent: Option<usize>,
	kind: Kind<'a, M>,
	/// Привязка двойного клика (ADR-0035): у узла, а не у элемента, — так она есть у любого
	/// контрола, встроенного и своего.
	on_double_click: Option<M>,
}

enum Kind<'a, M> {
	/// Контейнер: раскладывает детей, фокус не берёт; с рамкой — рисует её.
	Container {
		layout: Layout,
		block: Option<Block<'a>>,
		/// Стиль, накладываемый на рамку, когда фокус внутри.
		focus_style: Option<Style>,
		children: Vec<usize>,
	},
	Element(Box<dyn Element<'a, M> + 'a>),
}

impl<'a, M: 'a> Ui<'a, M> {
	fn single(kind: Kind<'a, M>) -> Self {
		Self {
			nodes: vec![Node {
				parent: None,
				kind,
				on_double_click: None,
			}],
			areas: Vec::new(),
			modal: None,
			hotkeys: None,
			clipboard: None,
			theme: None,
		}
	}

	/// Прозрачный контейнер: дети раскладываются по `layout`, детей больше ограничений —
	/// лишним пустая область (не рисуются, мышь не ловят).
	pub fn split(layout: Layout, children: Vec<Ui<'a, M>>) -> Self {
		Self::container(layout, None, children)
	}

	/// Контейнер с рамкой: дети раскладываются внутри `block.inner(area)`; стиль рамки при
	/// фокусе внутри — [`Ui::focus_style`].
	pub fn group(block: Block<'a>, layout: Layout, children: Vec<Ui<'a, M>>) -> Self {
		Self::container(layout, Some(block), children)
	}

	fn container(layout: Layout, block: Option<Block<'a>>, children: Vec<Ui<'a, M>>) -> Self {
		let mut ui = Self::single(Kind::Container {
			layout,
			block,
			focus_style: None,
			children: Vec::new(),
		});
		let mut roots = Vec::with_capacity(children.len());
		for child in children {
			// Модальный слой дочернего описания поднимается к корню (выпадающий список строит
			// его сам, ADR-0027); первый слой побеждает.
			if ui.modal.is_none() {
				ui.modal = child.modal;
			}
			// Карта сочетаний дочернего описания — так же (ADR-0029).
			if ui.hotkeys.is_none() {
				ui.hotkeys = child.hotkeys;
			}
			if ui.clipboard.is_none() {
				ui.clipboard = child.clipboard;
			}
			if ui.theme.is_none() {
				ui.theme = child.theme;
			}
			let offset = ui.nodes.len();
			roots.push(offset);
			for mut node in child.nodes {
				node.parent = Some(node.parent.map_or(0, |p| p + offset));
				if let Kind::Container { children, .. } = &mut node.kind {
					for index in children {
						*index += offset;
					}
				}
				ui.nodes.push(node);
			}
		}
		if let Kind::Container { children, .. } = &mut ui.nodes[0].kind {
			*children = roots;
		}
		ui
	}

	/// Свой контрол приложения: рисует себя сам, фокус и ввод — по трейту [`Control`].
	pub fn control<C: Control<M> + Drawable>(control: &'a mut C) -> Self {
		Self::single(Kind::Element(Box::new(Custom { control })))
	}

	/// Встроенный контрол: вид задаёт виджет-значение, поведение — состояние (ADR-0023);
	/// события состояния превращаются в сообщения по привязкам [`Ui::on_select`] и
	/// [`Ui::on_activate`].
	pub fn stateful<W, S>(widget: W, state: &'a mut S) -> Self
	where
		W: StatefulWidget<State = S> + 'a,
		S: Control<M> + WidgetEvents,
	{
		Self::single(Kind::Element(Box::new(Stateful::new(widget, state))))
	}

	/// Контрол, которым владеет описание (временная обёртка над состоянием приложения —
	/// содержимое выпадающего списка, ADR-0027). Флаги фокуса и области контрол обязан хранить
	/// в состоянии приложения, иначе они пропадут вместе с описанием.
	pub fn owned<C: Control<M> + Drawable + 'a>(control: C) -> Self {
		Self::single(Kind::Element(Box::new(Owned { control })))
	}

	/// Лист без состояния: рисуется, фокус и ввод не получает; мышь по нему — «не обработано».
	pub fn widget<W: Widget + 'a>(widget: W) -> Self {
		Self::single(Kind::Element(Box::new(Stateless {
			widget: Some(widget),
		})))
	}

	/// Стиль рамки контейнера [`Ui::group`], пока фокус внутри него; у других узлов не действует.
	pub fn focus_style(mut self, style: Style) -> Self {
		if let Kind::Container { focus_style, .. } = &mut self.nodes[0].kind {
			*focus_style = Some(style);
		}
		self
	}

	/// Сообщение при изменении выделения встроенного контрола ([`Ui::stateful`]); у других узлов
	/// не действует.
	pub fn on_select(mut self, to_message: impl Fn(usize) -> M + 'a) -> Self {
		if let Kind::Element(element) = &mut self.nodes[0].kind {
			element.bind_select(Box::new(to_message));
		}
		self
	}

	/// Сообщение при активации встроенного контрола ([`Ui::stateful`], кнопка); у других узлов не
	/// действует.
	pub fn on_activate(mut self, message: M) -> Self {
		if let Kind::Element(element) = &mut self.nodes[0].kind {
			element.bind_activate(message);
		}
		self
	}

	/// Сообщение при изменении значения поля ввода (ADR-0027); у других узлов не действует.
	pub fn on_change(mut self, to_message: impl Fn(String) -> M + 'a) -> Self {
		if let Kind::Element(element) = &mut self.nodes[0].kind {
			element.bind_change(Box::new(to_message));
		}
		self
	}

	/// Сообщение при подтверждении поля ввода `Enter` (ADR-0027); у других узлов не действует.
	pub fn on_submit(mut self, to_message: impl Fn(String) -> M + 'a) -> Self {
		if let Kind::Element(element) = &mut self.nodes[0].kind {
			element.bind_submit(Box::new(to_message));
		}
		self
	}

	/// Сообщение при переключении флажка пользователем (ADR-0031): аргумент — новое значение; у
	/// других узлов не действует.
	pub fn on_toggle(mut self, to_message: impl Fn(bool) -> M + 'a) -> Self {
		if let Kind::Element(element) = &mut self.nodes[0].kind {
			element.bind_toggle(Box::new(to_message));
		}
		self
	}

	/// Сообщение при изменении значения ползунка пользователем (ADR-0036): аргумент — новое
	/// значение; у других узлов не действует.
	pub fn on_value(mut self, to_message: impl Fn(i64) -> M + 'a) -> Self {
		if let Kind::Element(element) = &mut self.nodes[0].kind {
			element.bind_value(Box::new(to_message));
		}
		self
	}

	/// Сообщение на двойной клик по контролу (ADR-0035) — любому: встроенному ([`Ui::stateful`]),
	/// своему ([`Ui::control`]) или принадлежащему описанию ([`Ui::owned`]); у контейнеров и
	/// листов без состояния не действует. Публикуется после того, как второе нажатие доставлено
	/// контролу как обычное (первое тоже было обычным кликом), если контрол принимает двойной
	/// клик в этом месте ([`Control::accepts_double_click`]).
	pub fn on_double_click(mut self, message: M) -> Self {
		if let Kind::Element(element) = &self.nodes[0].kind {
			if element.node().is_some() {
				self.nodes[0].on_double_click = Some(message);
			}
		}
		self
	}

	/// Карта сочетаний экрана (ADR-0029): `tracker` и `map` живут в состоянии приложения.
	/// [`Ui::key`] кормит трекер каждым событием клавиши; нажатие, не принятое контролом под
	/// фокусом, сопоставляется с картой и публикует сообщение первой совпавшей привязки. При
	/// модальном слое действует только его карта ([`Modal::hotkeys`]).
	pub fn hotkeys(mut self, tracker: &'a mut HotkeyTracker, map: &'a HotkeyMap<M>) -> Self {
		self.hotkeys = Some(Hotkeys { tracker, map });
		self
	}

	/// Буфер обмена для команд правки (ADR-0030): объект живёт в состоянии приложения. Команды
	/// (`Ctrl+C/X/V/A` и классические с `Insert`/`Delete` — по физическим клавишам) идут
	/// контролу под фокусом экрана или модального слоя. Без подключённого буфера поле ввода
	/// команды всё равно поглощает, обмен с буфером — без действия.
	pub fn clipboard(mut self, clipboard: &'a mut dyn Clipboard) -> Self {
		self.clipboard = Some(clipboard);
		self
	}

	/// Тема оформления (ADR-0037): значение живёт в состоянии приложения. При отрисовке
	/// область описания получает базовый стиль темы (`ui.text` на `ui.background`), рамки
	/// контейнеров, виджеты, контролы и модальный слой — стили своих областей; стиль, заданный
	/// у виджета явно, сильнее темы.
	pub fn theme(mut self, theme: &'a Theme) -> Self {
		self.theme = Some(theme);
		self
	}

	/// Раздать области: корень — `area`, дети контейнеров — по их раскладке; области записываются
	/// в состояния контролов для попадания мыши.
	pub fn layout(&mut self, area: Rect) -> &mut Self {
		let count = self.nodes.len();
		self.areas.clear();
		self.areas.resize(count, Rect::default());
		if count > 0 {
			self.areas[0] = area;
		}
		for index in 0..count {
			let Kind::Container {
				layout,
				block,
				children,
				..
			} = &self.nodes[index].kind
			else {
				continue;
			};
			let own = self.areas[index];
			let inner = block.as_ref().map_or(own, |b| b.inner(own));
			let rects = layout.split(inner);
			for (slot, &child) in children.iter().enumerate() {
				self.areas[child] = rects.get(slot).copied().unwrap_or_default();
			}
		}
		for (node, &area) in self.nodes.iter_mut().zip(&self.areas) {
			if let Kind::Element(element) = &mut node.kind {
				if let Some(state) = element.node_mut() {
					state.area = area;
				}
			}
		}
		self
	}

	/// Разложить и нарисовать основной экран: рамки контейнеров (со стилем фокуса, если фокус
	/// внутри), затем контролы и листы в порядке объявления; пустые области пропускаются.
	/// Модальный слой здесь не рисуется — он идёт в наложение ([`Ui::draw_modal`]). Потребляет
	/// описание.
	pub fn render(self, area: Rect, buf: &mut ScreenBuffer) {
		if let Some(theme) = self.theme {
			buf.set_style(area, theme.base());
		}
		self.render_nodes(area, buf);
	}

	/// Отрисовка узлов без базового стиля области: содержимое модального слоя лежит на фоне
	/// самого слоя.
	fn render_nodes(mut self, area: Rect, buf: &mut ScreenBuffer) {
		// Пока открыт слой, фокус экрана не трогаем (ADR-0026): контрол под фокусом может
		// отсутствовать в описании (поле раскрытого списка рисуется снимком), и переназначение
		// отдало бы фокус первому кандидату.
		if self.modal.is_none() {
			self.ensure_focus();
		}
		self.layout(area);
		let inside = self.focus_inside();
		let theme = self.theme;
		for ((node, &area), &has_focus) in self.nodes.iter_mut().zip(&self.areas).zip(&inside) {
			if area.is_empty() {
				continue;
			}
			match &mut node.kind {
				Kind::Container {
					block: Some(block),
					focus_style,
					..
				} => {
					let mut block = block.clone();
					if let Some(theme) = theme {
						block.apply_theme(theme);
					}
					if has_focus {
						let themed = theme.map(Theme::focus_border).unwrap_or_default();
						block.patch_border_style(themed.patch(focus_style.unwrap_or_default()));
					}
					block.render(area, buf);
				}
				Kind::Container { .. } => {}
				Kind::Element(element) => element.render(area, buf, theme),
			}
		}
	}

	/// Для каждого узла — лежит ли фокус в его поддереве (у контрола — на нём самом).
	fn focus_inside(&self) -> Vec<bool> {
		let mut inside = vec![false; self.nodes.len()];
		let Some(mut index) = self.focused() else {
			return inside;
		};
		loop {
			inside[index] = true;
			match self.nodes[index].parent {
				Some(parent) => index = parent,
				None => return inside,
			}
		}
	}

	/// Контролы (узлы с состоянием) в порядке обхода.
	fn elements(&self) -> impl Iterator<Item = (usize, &(dyn Element<'a, M> + 'a))> {
		self.nodes
			.iter()
			.enumerate()
			.filter_map(|(index, node)| match &node.kind {
				Kind::Element(element) if element.node().is_some() => {
					Some((index, element.as_ref()))
				}
				_ => None,
			})
	}

	/// Слой закрыли кликом мимо: каждому контролу описания — `dismiss`.
	pub(super) fn dismiss_all(&mut self) {
		for node in &mut self.nodes {
			if let Kind::Element(element) = &mut node.kind {
				element.dismiss();
			}
		}
	}

	fn element_mut(&mut self, index: usize) -> &mut (dyn Element<'a, M> + 'a) {
		match &mut self.nodes[index].kind {
			Kind::Element(element) => element.as_mut(),
			Kind::Container { .. } => unreachable!("узел {index} — контейнер, а не контрол"),
		}
	}
}
