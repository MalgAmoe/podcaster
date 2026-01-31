defmodule PoddyclipBackendWeb.PageHTML do
  use PoddyclipBackendWeb, :html

  embed_templates "page_html/*"

  def format_date(datetime) do
    now = DateTime.utc_now()
    diff_days = Date.diff(DateTime.to_date(now), DateTime.to_date(datetime))
    time = Calendar.strftime(datetime, "%H:%M")

    cond do
      diff_days == 0 -> "Today at #{time}"
      diff_days == 1 -> "Yesterday at #{time}"
      diff_days < 7 -> "#{diff_days} days ago at #{time}"
      true -> Calendar.strftime(datetime, "%b %d, %Y at %H:%M")
    end
  end
end
