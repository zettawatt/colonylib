# Metadata Examples for ColonyLib

This directory contains examples of how to create JSON-LD metadata entries for files stored using ColonyLib. These examples demonstrate the proper structure and schema.org vocabulary to use when describing various types of digital content.

## Purpose

The intent of these metadata example files is to provide users of ColonyLib with clear, practical examples of how to create JSON-LD entries that properly describe their digital assets. By following these patterns, users can create rich, semantic metadata that makes their content more discoverable and interoperable.

## What is JSON-LD?

JSON-LD (JavaScript Object Notation for Linked Data) is a method of encoding linked data using JSON. It allows you to add semantic meaning to your data by using standardized vocabularies like schema.org.

## Schema.org Vocabulary

All examples in this directory use the schema.org vocabulary, which provides a comprehensive set of schemas for describing various types of content on the web. Schema.org is widely adopted and understood by search engines and other semantic web tools.

## File Structure

Each JSON-LD entry follows this basic structure:

```json
{
  "@context": {"schema": "http://schema.org/"},
  "@type": "schema:TypeName",
  "@id": "ant://[hash]",
  "schema:property": "value"
}
```

### Key Components:

- **@context**: Defines the vocabulary namespace (schema.org in our case)
- **@type**: Specifies the type of object being described
- **@id**: Unique identifier using the ANT protocol URI scheme
- **schema:properties**: Various properties that describe the object

## Content Types Covered

The examples demonstrate metadata creation for:

### Music Files
- **Type**: `schema:MediaObject`
- **Properties**: name, encodingFormat, description, creator
- **Example**: Audio files in MP3 format

### Video Files
- **Type**: `schema:VideoObject`
- **Properties**: name, encodingFormat, description, contentLocation, dateCreated, creator, contentSize
- **Example**: MP4 videos, documentaries, music videos

### Operating System Images
- **Type**: `schema:SoftwareApplication`
- **Properties**: name, operatingSystem, applicationCategory, processorRequirements, contentSize, dateCreated
- **Example**: Linux distribution ISO files

### Images
- **Type**: `schema:ImageObject`
- **Properties**: name, encodingFormat, description, dateCreated
- **Example**: JPEG images, photographs, artwork

### Books
- **Type**: `schema:Book`
- **Properties**: description, author
- **Example**: Digital book collections

### Software and Games
- **Type**: `schema:SoftwareSourceCode` (for scripts) or `schema:VideoGame` (for games) or `schema:SoftwareApplication` (for applications)
- **Properties**: name, programmingLanguage, description, applicationCategory, genre, operatingSystem
- **Example**: Shell scripts, games, applications

## Best Practices

1. **Use Appropriate Types**: Choose the most specific schema.org type that matches your content
2. **Include Essential Properties**: Always include at least name and description
3. **Use Standard Formats**: Follow established conventions for dates, file formats, etc.
4. **Be Descriptive**: Provide meaningful descriptions that help users understand the content
5. **Don't Fabricate Data**: Only include properties for which you have actual information

## Usage Guidelines

When creating your own JSON-LD metadata:

1. Start with the basic structure shown above
2. Choose the appropriate `@type` for your content
3. Use the ANT protocol URI scheme for the `@id` field with your file's hash
4. Add relevant schema.org properties based on your content type
5. Validate your JSON-LD using online tools if needed

## Genesis Pod Context

The examples in `genesis.md` represent the first collection of public immutable files known on the main Autonomi network, originally posted on the [Community Public File Directory](https://forum.autonomi.community/t/community-public-file-directory/41280) on the Autonomi Forum. These serve as historical examples and demonstrate real-world usage patterns.

## Further Reading

- [Schema.org Documentation](https://schema.org/)
- [JSON-LD Specification](https://json-ld.org/)
- [ColonyLib Documentation](../README.md)
