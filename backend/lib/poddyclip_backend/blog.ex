defmodule PoddyclipBackend.Blog do
  alias PoddyclipBackend.Blog.Post

  use NimblePublisher,
    build: Post,
    from: Application.app_dir(:poddyclip_backend, "priv/blog/**/*.md"),
    as: :posts,
    earmark_options: [code_class_prefix: "language-"],
    highlighters: []

  @posts Enum.sort_by(@posts, & &1.date, {:desc, Date})

  def all_posts, do: @posts

  def list_posts(locale \\ "en") do
    @posts
    |> Enum.group_by(& &1.slug)
    |> Enum.map(fn {_slug, posts} ->
      Enum.find(posts, &(&1.locale == locale)) ||
        Enum.find(posts, &(&1.locale == "en"))
    end)
    |> Enum.reject(&is_nil/1)
    |> Enum.sort_by(& &1.date, {:desc, Date})
  end

  def get_post(slug, locale \\ "en") do
    Enum.find(@posts, &(&1.slug == slug and &1.locale == locale)) ||
      Enum.find(@posts, &(&1.slug == slug and &1.locale == "en"))
  end
end
