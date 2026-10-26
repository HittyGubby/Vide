import type { SearchResultItem } from "$lib/types";

class SearchPageState {
  query = $state("");
  activePlatform = $state("");
  results = $state<SearchResultItem[]>([]);
  loading = $state(false);
  error = $state<string | null>(null);
  searched = $state(false);
  urlMode = $state(false);
}

export const searchPage = new SearchPageState();
