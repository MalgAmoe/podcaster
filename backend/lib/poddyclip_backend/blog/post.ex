defmodule PoddyclipBackend.Blog.Post do
  @enforce_keys [:id, :slug, :title, :body, :description, :date, :locale]
  defstruct [:id, :slug, :title, :body, :description, :date, :locale, :image]

  def build(filename, attrs, body) do
    [locale, slug_file] = filename |> Path.split() |> Enum.take(-2)
    slug = Path.rootname(slug_file)

    struct!(__MODULE__,
      id: "#{locale}/#{slug}",
      slug: slug,
      locale: locale,
      title: Map.fetch!(attrs, :title),
      body: body,
      description: Map.fetch!(attrs, :description),
      date: Map.fetch!(attrs, :date),
      image: Map.get(attrs, :image)
    )
  end
end
