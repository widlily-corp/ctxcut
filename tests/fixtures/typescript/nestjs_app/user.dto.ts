/**
 * Data Transfer Objects for NestJS user endpoints with class-validator decorators.
 */

export function IsString(): PropertyDecorator {
  return () => {};
}

export function IsEmail(): PropertyDecorator {
  return () => {};
}

export function IsOptional(): PropertyDecorator {
  return () => {};
}

export function MinLength(min: number): PropertyDecorator {
  return () => {};
}

export class CreateUserDto {
  @IsString()
  username!: string;

  @IsEmail()
  email!: string;

  @IsString()
  @MinLength(8)
  password!: string;

  @IsOptional()
  @IsString()
  displayName?: string;
}

export class UpdateUserDto {
  @IsOptional()
  @IsEmail()
  email?: string;

  @IsOptional()
  @IsString()
  displayName?: string;

  @IsOptional()
  @IsString()
  bio?: string;
}

export class UserResponseDto {
  id!: string;
  username!: string;
  email!: string;
  displayName?: string;
  createdAt!: Date;
}
