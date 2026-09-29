import { insert, t, type Dictionary } from "intlayer";

const content = {
  key: "history",
  content: {
    noHistory: t({
      en: "No connection history yet",
      fr: "Pas encore d'historique de connexion",
      de: "Noch kein Verbindungsverlauf",
      es: "Sin historial de conexiones aún",
      ru: "Нет истории подключений",
    }),
    colLastPlayed: t({
      en: "Last played",
      fr: "Dernière partie",
      de: "Zuletzt gespielt",
      es: "Última partida",
      ru: "Последняя игра",
    }),
    remove: t({
      en: "Remove from history",
      fr: "Retirer de l'historique",
      de: "Aus Verlauf entfernen",
      es: "Quitar del historial",
      ru: "Удалить из истории",
    }),
    clearAll: t({
      en: "Clear all history",
      fr: "Effacer tout l'historique",
      de: "Gesamten Verlauf löschen",
      es: "Limpiar todo el historial",
      ru: "Очистить всю историю",
    }),
    clearAllTitle: t({
      en: "Permanently remove all entries from connection history",
      fr: "Supprimer définitivement toutes les entrées de l'historique",
      de: "Alle Einträge aus dem Verbindungsverlauf dauerhaft entfernen",
      es: "Eliminar permanentemente todas las entradas del historial de conexiones",
      ru: "Навсегда удалить все записи из истории подключений",
    }),
    removeTitle: t({
      en: "Remove Entry",
      fr: "Retirer l'entrée",
      de: "Eintrag entfernen",
      es: "Quitar entrada",
      ru: "Удаление записи",
    }),
    removeMessage: insert(
      t({
        en: "Remove '{{name}}' from history?",
        fr: "Retirer '{{name}}' de l'historique ?",
        de: "'{{name}}' aus dem Verlauf entfernen?",
        es: "¿Quitar '{{name}}' del historial?",
        ru: "Удалить '{{name}}' из истории?",
      }),
    ),
    removed: t({
      en: "Removed from history",
      fr: "Retiré de l'historique",
      de: "Aus Verlauf entfernt",
      es: "Quitado del historial",
      ru: "Удалено из истории",
    }),
    clearTitle: t({
      en: "Clear History",
      fr: "Effacer l'historique",
      de: "Verlauf löschen",
      es: "Limpiar historial",
      ru: "Очистка истории",
    }),
    clearMessage: insert(
      t({
        en: "Clear all {{count}} history entries?",
        fr: "Effacer toutes les {{count}} entrées de l'historique ?",
        de: "Alle {{count}} Verlaufseinträge löschen?",
        es: "¿Limpiar todas las {{count}} entradas del historial?",
        ru: "Очистить все {{count}} записей истории?",
      }),
    ),
    cleared: t({
      en: "History cleared",
      fr: "Historique effacé",
      de: "Verlauf gelöscht",
      es: "Historial limpiado",
      ru: "История очищена",
    }),
    addedToFavorites: insert(
      t({
        en: "Added {{name}} to favorites",
        fr: "{{name}} ajouté aux favoris",
        de: "{{name}} zu Favoriten hinzugefügt",
        es: "Añadido {{name}} a favoritos",
        ru: "Добавлено {{name}} в избранное",
      }),
    ),
    statEntries: t({
      en: "Servers played",
      fr: "Serveurs joués",
      de: "Gespielte Server",
      es: "Servidores jugados",
      ru: "Серверов сыграно",
    }),
    statLastPlayed: t({
      en: "Last session",
      fr: "Dernière session",
      de: "Letzte Sitzung",
      es: "Última sesión",
      ru: "Последняя сессия",
    }),
    statThisWeek: t({
      en: "This week",
      fr: "Cette semaine",
      de: "Diese Woche",
      es: "Esta semana",
      ru: "За неделю",
    }),
    noHistoryHint: t({
      en: "Every server you join is kept here, newest first, so you can go back in one click.",
      fr: "Chaque serveur rejoint est gardé ici, du plus récent au plus ancien, pour y revenir en un clic.",
      de: "Jeder Server, dem du beitrittst, landet hier – neueste zuerst –, damit du mit einem Klick zurückkehrst.",
      es: "Cada servidor al que te unes se guarda aquí, del más reciente al más antiguo, para volver con un clic.",
      ru: "Каждый сервер, на который вы заходили, сохраняется здесь — новые сверху, чтобы вернуться в один клик.",
    }),
    sortRecent: t({
      en: "Most recent",
      fr: "Plus récents",
      de: "Neueste",
      es: "Más recientes",
      ru: "Недавние",
    }),
  },
} satisfies Dictionary;

export default content;
