import { t, type Dictionary } from "intlayer";

// The words the interface's own parts say, whatever view they are in.
const content = {
  key: "common",
  content: {
    close: t({ en: "Close", fr: "Fermer", de: "Schließen", es: "Cerrar", ru: "Закрыть" }),
    chartKeys: t({
      en: "Arrow keys move from cell to cell.",
      fr: "Les flèches passent d'une case à l'autre.",
      de: "Die Pfeiltasten wechseln von Feld zu Feld.",
      es: "Las flechas pasan de una casilla a otra.",
      ru: "Стрелки переходят между ячейками.",
    }),
    cancel: t({ en: "Cancel", fr: "Annuler", de: "Abbrechen", es: "Cancelar", ru: "Отмена" }),
    confirm: t({
      en: "Confirm",
      fr: "Confirmer",
      de: "Bestätigen",
      es: "Confirmar",
      ru: "Подтвердить",
    }),
    save: t({ en: "Save", fr: "Enregistrer", de: "Speichern", es: "Guardar", ru: "Сохранить" }),
    clear: t({ en: "Clear", fr: "Effacer", de: "Leeren", es: "Borrar", ru: "Очистить" }),
    clearSearch: t({
      en: "Clear the search",
      fr: "Effacer la recherche",
      de: "Suche leeren",
      es: "Borrar la búsqueda",
      ru: "Очистить поиск",
    }),
    search: t({ en: "Search", fr: "Rechercher", de: "Suchen", es: "Buscar", ru: "Поиск" }),
    refresh: t({
      en: "Refresh",
      fr: "Actualiser",
      de: "Aktualisieren",
      es: "Actualizar",
      ru: "Обновить",
    }),
    retry: t({
      en: "Retry",
      fr: "Réessayer",
      de: "Erneut versuchen",
      es: "Reintentar",
      ru: "Повторить",
    }),
    copy: t({ en: "Copy", fr: "Copier", de: "Kopieren", es: "Copiar", ru: "Копировать" }),
    copied: t({ en: "Copied", fr: "Copié", de: "Kopiert", es: "Copiado", ru: "Скопировано" }),
    open: t({ en: "Open", fr: "Ouvrir", de: "Öffnen", es: "Abrir", ru: "Открыть" }),
    loading: t({
      en: "Loading…",
      fr: "Chargement…",
      de: "Wird geladen…",
      es: "Cargando…",
      ru: "Загрузка…",
    }),
    none: t({ en: "None", fr: "Aucun", de: "Keine", es: "Ninguno", ru: "Нет" }),
    all: t({ en: "All", fr: "Tous", de: "Alle", es: "Todos", ru: "Все" }),
    yes: t({ en: "Yes", fr: "Oui", de: "Ja", es: "Sí", ru: "Да" }),
    no: t({ en: "No", fr: "Non", de: "Nein", es: "No", ru: "Нет" }),
    live: t({ en: "Live", fr: "Direct", de: "Live", es: "En vivo", ru: "Онлайн" }),
    filterLines: t({
      en: "Filter lines…",
      fr: "Filtrer les lignes…",
      de: "Zeilen filtern…",
      es: "Filtrar líneas…",
      ru: "Фильтр строк…",
    }),
    unknown: t({
      en: "Unknown",
      fr: "Inconnu",
      de: "Unbekannt",
      es: "Desconocido",
      ru: "Неизвестно",
    }),
  },
} satisfies Dictionary;

export default content;
