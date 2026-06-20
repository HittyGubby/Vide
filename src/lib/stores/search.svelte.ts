const HISTORY_KEY = "vide_search_history";
const MAX_ITEMS = 20;

class SearchStore {
  history = $state<string[]>([]);

  constructor() {
    this.load();
  }

  load() {
    try {
      const data = JSON.parse(localStorage.getItem(HISTORY_KEY) || "[]");
      this.history = Array.isArray(data) ? data.slice(0, MAX_ITEMS) : [];
    } catch {
      this.history = [];
    }
  }

  add(query: string) {
    const trimmed = query.trim();
    if (!trimmed) return;
    this.history = this.history.filter((q) => q !== trimmed);
    this.history = [trimmed, ...this.history].slice(0, MAX_ITEMS);
    this.save();
  }

  remove(query: string) {
    this.history = this.history.filter((q) => q !== query);
    this.save();
  }

  clear() {
    this.history = [];
    this.save();
  }

  private save() {
    try {
      localStorage.setItem(HISTORY_KEY, JSON.stringify(this.history));
    } catch {
      /* storage full */
    }
  }
}

export const searchStore = new SearchStore();
